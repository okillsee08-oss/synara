use anyhow::Result;use synara_process::{ManagedProcess,ProcessSpec};
pub struct Terminal{pub process:ManagedProcess}
impl Terminal{pub async fn spawn(shell:String,cwd:Option<String>)->Result<Self>{Ok(Self{process:ManagedProcess::spawn(ProcessSpec{program:shell,args:Vec::new(),cwd}).await?})}}
