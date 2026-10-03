use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Transport {
    Stdio,
    Http,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: Transport,
    pub endpoint: Option<String>,
}

impl McpServerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            anyhow::bail!("MCP server name cannot be empty");
        }
        match self.transport {
            Transport::Http
                if self
                    .endpoint
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty() =>
            {
                anyhow::bail!("HTTP MCP server requires an endpoint")
            }
            Transport::Stdio
                if self
                    .endpoint
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty() =>
            {
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

impl JsonRpcResponse {
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn into_result(self) -> Result<Value> {
        match self.error {
            Some(error) => anyhow::bail!("MCP JSON-RPC error: {error}"),
            None => Ok(self.result.unwrap_or(Value::Null)),
        }
    }
}

pub struct StdioClient {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
    timeout: Duration,
}

impl StdioClient {
    pub async fn spawn(program: &str, args: &[String]) -> Result<Self> {
        Self::spawn_with_timeout(program, args, Duration::from_secs(60)).await
    }

    pub async fn spawn_with_timeout(
        program: &str,
        args: &[String],
        timeout: Duration,
    ) -> Result<Self> {
        if program.trim().is_empty() {
            anyhow::bail!("MCP server program cannot be empty");
        }
        if timeout.is_zero() {
            anyhow::bail!("MCP timeout must be greater than zero");
        }
        let mut child = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn MCP server {program}"))?;
        let stdin = child.stdin.take().context("MCP stdin unavailable")?;
        let stdout = child.stdout.take().context("MCP stdout unavailable")?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            timeout,
        })
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: self.next_id,
            method: method.into(),
            params,
        };
        self.next_id += 1;
        let request_id = request.id;

        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;

        tokio::time::timeout(self.timeout, async {
            loop {
                let mut response = String::new();
                let bytes = self.stdout.read_line(&mut response).await?;
                if bytes == 0 {
                    anyhow::bail!("MCP server closed stdout without a matching response");
                }
                if response.trim().is_empty() {
                    continue;
                }

                let value: Value = serde_json::from_str(response.trim())
                    .context("invalid MCP JSON-RPC message")?;

                if value.get("id").is_none() {
                    continue;
                }

                let parsed: JsonRpcResponse =
                    serde_json::from_value(value).context("invalid MCP JSON-RPC response")?;
                if parsed.jsonrpc != "2.0" {
                    anyhow::bail!("unsupported JSON-RPC version: {}", parsed.jsonrpc);
                }
                if parsed.id != request_id {
                    continue;
                }
                return Ok(parsed);
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("MCP request timed out after {:?}", self.timeout))?
    }

    pub async fn initialize_legacy(
        &mut self,
        protocol_version: &str,
        client_name: &str,
        client_version: &str,
    ) -> Result<JsonRpcResponse> {
        self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": protocol_version,
                "capabilities": {},
                "clientInfo": {
                    "name": client_name,
                    "version": client_version
                }
            }),
        )
        .await
    }

    pub async fn discover_server(&mut self) -> Result<JsonRpcResponse> {
        self.request("server/discover", Value::Null).await
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self
            .request("tools/list", Value::Null)
            .await?
            .into_result()?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("MCP tools/list returned no tools array"))?;

        Ok(tools
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<ToolResult> {
        let result = self
            .request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await?
            .into_result()?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

pub struct HttpClient {
    client: reqwest::Client,
    endpoint: String,
    next_id: u64,
    timeout: Duration,
}

impl HttpClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self::with_timeout(endpoint, Duration::from_secs(60))
    }

    pub fn with_timeout(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
            next_id: 1,
            timeout,
        }
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: self.next_id,
            method: method.into(),
            params,
        };
        self.next_id += 1;
        let response = self
            .client
            .post(&self.endpoint)
            .timeout(self.timeout)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<JsonRpcResponse>()
            .await?;
        Ok(response)
    }

    pub async fn initialize_legacy(
        &mut self,
        protocol_version: &str,
        client_name: &str,
        client_version: &str,
    ) -> Result<JsonRpcResponse> {
        self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": protocol_version,
                "capabilities": {},
                "clientInfo": {
                    "name": client_name,
                    "version": client_version
                }
            }),
        )
        .await
    }

    pub async fn discover_server(&mut self) -> Result<JsonRpcResponse> {
        self.request("server/discover", Value::Null).await
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self
            .request("tools/list", Value::Null)
            .await?
            .into_result()?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("MCP tools/list returned no tools array"))?;
        Ok(tools
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<ToolResult> {
        let result = self
            .request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await?
            .into_result()?;
        Ok(serde_json::from_value(result)?)
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    #[test]
    fn validates_server_config() {
        assert!(
            McpServerConfig {
                name: "".into(),
                transport: Transport::Stdio,
                endpoint: None,
            }
            .validate()
            .is_err()
        );
        assert!(
            McpServerConfig {
                name: "x".into(),
                transport: Transport::Http,
                endpoint: None,
            }
            .validate()
            .is_err()
        );
        assert!(
            McpServerConfig {
                name: "x".into(),
                transport: Transport::Stdio,
                endpoint: None,
            }
            .validate()
            .is_ok()
        );
    }
}
