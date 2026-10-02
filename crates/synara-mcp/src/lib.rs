use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Transport { Stdio, Http }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: Transport,
    pub endpoint: Option<String>,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

pub struct StdioClient {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl StdioClient {
    pub async fn spawn(program: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn MCP server {program}"))?;
        let stdin = child.stdin.take().context("MCP stdin unavailable")?;
        let stdout = child.stdout.take().context("MCP stdout unavailable")?;
        Ok(Self { child, stdin, stdout: BufReader::new(stdout), next_id: 1 })
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: self.next_id,
            method: method.into(),
            params,
        };
        self.next_id += 1;
        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;

        let mut response = String::new();
        self.stdout.read_line(&mut response).await?;
        if response.trim().is_empty() {
            anyhow::bail!("MCP server closed stdout without a response");
        }
        Ok(serde_json::from_str(response.trim())?)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}
