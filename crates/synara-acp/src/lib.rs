use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcpRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcpResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcpEvent {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub struct AcpSession {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl AcpSession {
    pub async fn spawn(program: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn ACP provider {program}"))?;
        let stdin = child.stdin.take().context("ACP stdin unavailable")?;
        let stdout = child.stdout.take().context("ACP stdout unavailable")?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<AcpResponse> {
        let request = AcpRequest {
            jsonrpc: "2.0".into(),
            id: self.next_id,
            method: method.into(),
            params,
        };
        self.next_id += 1;
        let mut encoded = serde_json::to_vec(&request)?;
        encoded.push(b'\n');
        self.stdin.write_all(&encoded).await?;
        self.stdin.flush().await?;

        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).await?;
            if line.trim().is_empty() {
                anyhow::bail!("ACP provider closed stdout");
            }
            let value: Value = serde_json::from_str(line.trim())?;
            if value.get("id").and_then(Value::as_u64) == Some(request.id) {
                return Ok(serde_json::from_value(value)?);
            }
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}
