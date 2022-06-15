//! 回话消息句柄
//! 
use async_trait::async_trait;
#[async_trait]
pub trait SendHandler<T>: Clone where T: Send + Sync{
    fn send_sync(&self, msg: T) -> anyhow::Result<()>;
    async fn send_async(&self, msg: T) -> anyhow::Result<()>;
}
#[async_trait]
pub trait RecvHandler<T> where T: Send + Sync{
    fn try_recv(&mut self) -> Result<T,()>;
    async fn recv_async(&mut self) -> anyhow::Result<T>;
}
pub struct SocketHandler<T, R, E, K> where T: SendHandler<E>, R: RecvHandler<K>, E: Send + Sync, K: Send + Sync{
    pub(crate) sender: T, 
    pub(crate) receiver: R,
    p: std::marker::PhantomData<(E,K)>,
}
impl<T, R, E, K> Clone for SocketHandler<T, R, E, K> where T: SendHandler<E>, R: RecvHandler<K> + Clone, E: Send + Sync, K: Send + Sync{
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone(), receiver: self.receiver.clone(), p: std::marker::PhantomData }
    }
}
impl<T, R, E, K>  SocketHandler<T, R, E, K>  where T: SendHandler<E>, R: RecvHandler<K>, E: Send + Sync, K: Send + Sync{
    #[inline]
    pub fn new(t: T, r: R) -> Self{
        Self{sender: t, receiver: r, p: std::marker::PhantomData}
    }
    #[inline]
    pub fn send(&self, msg: E) -> anyhow::Result<()>{
        self.sender.send_sync(msg)
    }
    #[allow(unused)]
    #[inline]
    pub async fn send_async(&self, msg: E) -> anyhow::Result<()>{
        self.sender.send_async(msg).await
    }
    #[inline]
    pub fn try_recv(&mut self) -> Result<K,()> {
        self.receiver.try_recv()
    }
    #[inline]
    pub async fn recv_async(&mut self) -> anyhow::Result<K>{
        self.receiver.recv_async().await
    }
}
#[async_trait]
impl<T> SendHandler<T> for crossbeam::channel::Sender<T> where T: Send + Sync{
    #[inline]
    fn send_sync(&self, msg: T) -> anyhow::Result<()> {
        self.send(msg).map_err(|_| crate::error::send_err())?;
        Ok(())
    }
    #[inline]
    async fn send_async(&self, msg: T) -> anyhow::Result<()> {
        self.send(msg).map_err(|_| crate::error::send_err())?;
        Ok(())
    }
}

#[async_trait]
impl<T> SendHandler<T> for tokio::sync::mpsc::UnboundedSender<T>  where T: Send + Sync + std::fmt::Debug + 'static{
    #[inline]
    fn send_sync(&self, msg: T) -> anyhow::Result<()> {
        self.send(msg)?;
        Ok(())
    }
    #[inline]
    async fn send_async(&self, msg: T) -> anyhow::Result<()> {
        self.send(msg)?;
        Ok(())
    }
}
#[async_trait]
impl<T> RecvHandler<T> for tokio::sync::mpsc::UnboundedReceiver<T> where T: Send + Sync{
    fn try_recv(&mut self) -> Result<T,()> {
        match self.blocking_recv(){
            Some(v) => Ok(v),
            None => {
                let e = Err(())?;
                Ok(e)
            },
        }
    }
    #[inline]
    async fn recv_async(&mut self) -> anyhow::Result<T> {
        match  self.recv().await {
            Some(v) => Ok(v),
            None => crate::error::broken_pipe(),
        }
    }
}
#[async_trait]
impl<T> RecvHandler<T> for crossbeam::channel::Receiver<T> where T: Send + Sync{
    #[inline]
    fn try_recv(&mut self) -> Result<T,()> {
        crossbeam::channel::Receiver::try_recv(self).map_err(|_| ())
    }
    #[inline]
    async fn recv_async(&mut self) -> anyhow::Result<T> {
        let ret = self.recv()?;
        Ok(ret)
    }
}