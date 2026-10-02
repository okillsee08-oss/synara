use std::sync::Arc;use tokio::sync::broadcast;use synara_protocol::Envelope;use serde::Serialize;
pub struct EventBus<T:Clone+Send+'static>{tx:broadcast::Sender<Arc<Envelope<T>>>}
impl<T:Clone+Send+'static> EventBus<T>{pub fn new(capacity:usize)->Self{let(tx,_)=broadcast::channel(capacity);Self{tx}}pub fn publish(&self,event:Envelope<T>){let _=self.tx.send(Arc::new(event));}pub fn subscribe(&self)->broadcast::Receiver<Arc<Envelope<T>>>{self.tx.subscribe()}}
