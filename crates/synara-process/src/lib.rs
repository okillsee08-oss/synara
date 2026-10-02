use anyhow::{bail, Result};
use std::process::Stdio;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

#[derive(Clone, Debug)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

impl ProcessSpec {
    pub fn validate(&self) -> Result<()> {
        if self.program.trim().is_empty() {
            bail!("process program must not be empty");
        }
        Ok(())
    }
}

pub struct ManagedProcess {
    pub child: Child,
}

impl ManagedProcess {
    pub async fn spawn(s: ProcessSpec) -> Result<Self> {
        s.validate()?;

        let mut command = Command::new(&s.program);
        command
            .args(&s.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = s.cwd {
            command.current_dir(cwd);
        }

        Ok(Self {
            child: command.spawn()?,
        })
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub async fn is_finished(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }

    pub async fn terminate(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?;
        }
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        Ok(self.child.wait().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_program_is_rejected_before_spawn() {
        let spec = ProcessSpec {
            program: "  ".into(),
            args: vec![],
            cwd: None,
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn non_empty_program_is_valid() {
        let spec = ProcessSpec {
            program: "synara-test-program".into(),
            args: vec![],
            cwd: None,
        };
        assert!(spec.validate().is_ok());
    }
}
