use serde::{Deserialize,Serialize};
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct WsNegotiation{pub epoch:u64,pub negotiated_revision:u32,pub server_instance_id:String,pub capabilities:Vec<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Envelope<T>{pub revision:u32,pub sequence:u64,pub payload:T}
