use anyhow::Result;
use std::process::Stdio;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

#[derive(Clone, Debug)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

pub struct ManagedProcess {
    pub child: Child,
}

impl ManagedProcess {
    pub async fn spawn(s: ProcessSpec) -> Result<Self> {
        let mut command = Command::new(s.program);
        command
            .args(s.args)
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

    pub async fn terminate(&mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        Ok(self.child.wait().await?)
    }
}
