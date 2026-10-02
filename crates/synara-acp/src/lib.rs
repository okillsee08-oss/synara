use serde::{Deserialize,Serialize};use serde_json::Value;
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct AcpRequest{pub id:u64,pub method:String,pub params:Value}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct AcpResponse{pub id:u64,pub result:Option<Value>,pub error:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct AcpEvent{pub method:String,pub params:Value}
