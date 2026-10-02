use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{Instant, sleep},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            delay: Duration::from_secs(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub interval: Duration,
    pub run_immediately: bool,
    pub retry: RetryPolicy,
}

pub struct AutomationHandle {
    pub id: String,
    cancel: CancellationToken,
    join: JoinHandle<()>,
}

impl AutomationHandle {
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub async fn join(self) -> Result<(), tokio::task::JoinError> {
        self.join.await
    }
}

#[derive(Clone, Default)]
pub struct Scheduler {
    jobs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn wait(delay: Duration) {
        sleep(delay).await
    }

    pub fn deadline(delay: Duration) -> Instant {
        Instant::now() + delay
    }

    pub async fn schedule<F, Fut>(
        &self,
        id: impl Into<String>,
        schedule: Schedule,
        job: F,
    ) -> AutomationHandle
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let id = id.into();
        if let Some(existing) = self.jobs.lock().await.remove(&id) {
            existing.cancel();
        }

        let cancel = CancellationToken::new();
        self.jobs.lock().await.insert(id.clone(), cancel.clone());
        let jobs = Arc::clone(&self.jobs);
        let task_id = id.clone();

        let join = tokio::spawn(async move {
            if schedule.run_immediately {
                let _ = run_with_retry(&schedule.retry, &job, &cancel).await;
            }

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = sleep(schedule.interval) => {
                        if cancel.is_cancelled() {
                            break;
                        }
                        let _ = run_with_retry(&schedule.retry, &job, &cancel).await;
                    }
                }
            }

            jobs.lock().await.remove(&task_id);
        });

        AutomationHandle { id, cancel, join }
    }
}

async fn run_with_retry<F, Fut>(
    policy: &RetryPolicy,
    job: &F,
    cancel: &CancellationToken,
) -> Result<(), String>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    let attempts = policy.max_attempts.max(1);
    let mut last_error = None;

    for attempt in 1..=attempts {
        if cancel.is_cancelled() {
            return Err("automation cancelled".into());
        }

        match job().await {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    tokio::select! {
                        _ = cancel.cancelled() => return Err("automation cancelled".into()),
                        _ = sleep(policy.delay) => {}
                    }
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "automation failed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn retries_until_success() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_for_job = attempts.clone();
        let scheduler = Scheduler::new();
        let handle = scheduler
            .schedule(
                "retry-test",
                Schedule {
                    interval: Duration::from_secs(60),
                    run_immediately: true,
                    retry: RetryPolicy {
                        max_attempts: 3,
                        delay: Duration::from_millis(1),
                    },
                },
                move || {
                    let attempts = attempts_for_job.clone();
                    async move {
                        let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                        if count < 2 {
                            Err("try again".into())
                        } else {
                            Ok(())
                        }
                    }
                },
            )
            .await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        handle.cancel();
        handle.join().await.unwrap();
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }
}
