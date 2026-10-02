use anyhow::Result;use tokio::task::JoinHandle;use tokio_util::sync::CancellationToken;
pub struct Runtime{pub cancellation:CancellationToken,handles:Vec<JoinHandle<()>>}
impl Runtime{pub fn new()->Self{Self{cancellation:CancellationToken::new(),handles:Vec::new()}}pub fn spawn<F>(&mut self,f:F)where F:std::future::Future<Output=()>+Send+'static{self.handles.push(tokio::spawn(f));}pub async fn shutdown(mut self)->Result<()>{self.cancellation.cancel();for h in self.handles.drain(..){let _=h.await;}Ok(())}}
