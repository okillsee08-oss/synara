use anyhow::Result;
#[derive(Clone,Debug)]pub struct BrowserSession{pub id:String,pub url:Option<String>}
pub trait BrowserDriver:Send+Sync{fn navigate(&self,url:&str)->Result<()>;fn screenshot(&self)->Result<Vec<u8>>;}
