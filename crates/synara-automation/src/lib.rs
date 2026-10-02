use std::time::Duration;
use tokio::time::{Instant, sleep};
pub struct Scheduler;
impl Scheduler {
    pub async fn wait(delay: Duration) {
        sleep(delay).await
    }
    pub fn deadline(delay: Duration) -> Instant {
        Instant::now() + delay
    }
}
