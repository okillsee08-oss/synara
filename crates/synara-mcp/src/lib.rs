use serde::{Deserialize,Serialize};use serde_json::Value;
#[derive(Clone,Debug,Serialize,Deserialize)]pub enum Transport{Stdio,Http}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct McpServerConfig{pub name:String,pub transport:Transport,pub endpoint:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct McpTool{pub name:String,pub description:Option<String>,pub input_schema:Value}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct ToolResult{pub content:Vec<Value>,pub is_error:bool}
