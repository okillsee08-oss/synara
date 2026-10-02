use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;

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

impl AcpResponse {
    pub fn into_result(self) -> Result<Value> {
        match self.error {
            Some(error) => anyhow::bail!("ACP JSON-RPC error: {error}"),
            None => Ok(self.result.unwrap_or(Value::Null)),
        }
    }
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
    timeout: Duration,
    events: broadcast::Sender<AcpEvent>,
}

impl AcpSession {
    pub async fn spawn(program: &str, args: &[String]) -> Result<Self> {
        Self::spawn_with_timeout(program, args, Duration::from_secs(60)).await
    }

    pub async fn spawn_with_timeout(
        program: &str,
        args: &[String],
        timeout: Duration,
    ) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn ACP provider {program}"))?;
        let stdin = child.stdin.take().context("ACP stdin unavailable")?;
        let stdout = child.stdout.take().context("ACP stdout unavailable")?;
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            timeout,
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AcpEvent> {
        self.events.subscribe()
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<AcpResponse> {
        let request = AcpRequest {
            jsonrpc: "2.0".into(),
            id: self.next_id,
            method: method.into(),
            params,
        };
        self.next_id += 1;
        let request_id = request.id;

        let mut encoded = serde_json::to_vec(&request)?;
        encoded.push(b'\n');
        self.stdin.write_all(&encoded).await?;
        self.stdin.flush().await?;

        let events = self.events.clone();
        let timeout = self.timeout;
        tokio::time::timeout(timeout, async {
            loop {
                let mut line = String::new();
                let bytes = self.stdout.read_line(&mut line).await?;
                if bytes == 0 {
                    anyhow::bail!("ACP provider closed stdout");
                }
                if line.trim().is_empty() {
                    continue;
                }

                let value: Value = serde_json::from_str(line.trim())
                    .context("invalid ACP JSON-RPC message")?;

                if value.get("id").is_some() {
                    let id = value.get("id").and_then(Value::as_u64);
                    if id != Some(request_id) {
                        continue;
                    }
                    let response: AcpResponse = serde_json::from_value(value)?;
                    if response.jsonrpc != "2.0" {
                        anyhow::bail!("unsupported ACP JSON-RPC version: {}", response.jsonrpc);
                    }
                    return Ok(response);
                }

                let event: AcpEvent = serde_json::from_value(value)?;
                if event.jsonrpc != "2.0" {
                    anyhow::bail!("unsupported ACP JSON-RPC version: {}", event.jsonrpc);
                }
                let _ = events.send(event);
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("ACP request timed out after {:?}", self.timeout))?
    }

    pub async fn initialize(
        &mut self,
        protocol_version: &str,
        client_name: &str,
        client_version: &str,
        capabilities: Value,
    ) -> Result<Value> {
        self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": protocol_version,
                "capabilities": capabilities,
                "clientInfo": {
                    "name": client_name,
                    "version": client_version
                }
            }),
        )
        .await?
        .into_result()
    }

    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.request(method, params).await?.into_result()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_error_is_surfaceable() {
        let response = AcpResponse {
            jsonrpc: "2.0".into(),
            id: 1,
            result: None,
            error: Some(serde_json::json!({"code": -1, "message": "boom"})),
        };
        assert!(response.into_result().is_err());
    }
}
