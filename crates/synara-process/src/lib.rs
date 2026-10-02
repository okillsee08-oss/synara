use anyhow::Result;use std::process::Stdio;use tokio::process::{Child,Command};
#[derive(Clone,Debug)]pub struct ProcessSpec{pub program:String,pub args:Vec<String>,pub cwd:Option<String>}
pub struct ManagedProcess{pub child:Child}
impl ManagedProcess{pub async fn spawn(s:ProcessSpec)->Result<Self>{let mut c=Command::new(s.program);c.args(s.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());if let Some(cwd)=s.cwd{c.current_dir(cwd);}Ok(Self{child:c.spawn()?})}pub async fn terminate(&mut self)->Result<()>{let _=self.child.kill().await;Ok(())}}
