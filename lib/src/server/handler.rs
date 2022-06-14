#![allow(type_alias_bounds)]
use std::{
    fmt::Debug, 
//    task::Waker,
};

use crate::{SessionTransport, SocketMessage};

use super::{socket_handler::{RecvHandler, SendHandler, SocketHandler}, session::TransferTemplate};

pub type Transporter<S: Sized + Debug + Send + Sync + 'static> = tokio::sync::mpsc::UnboundedSender<SessionTransport<S>>;
pub type TransportReceiver<S: Sized + Debug + Send + Sync + 'static> = tokio::sync::mpsc::UnboundedReceiver<SessionTransport<S>>;
pub type AsyncSocketHandler<S: Sized + Debug + Send + Sync + 'static> = tokio::sync::mpsc::UnboundedReceiver<SocketMessage<S>>;
pub type AsyncSocketSendHandler<S: Sized + Debug + Send + Sync + 'static> = tokio::sync::mpsc::UnboundedSender<SocketMessage<S>>;
pub type SyncSocketHandler<S: Sized + Debug + Send + Sync + 'static> = crossbeam::channel::Receiver<SocketMessage<S>>;

pub type SyncSessionHandler<S: Sized + Debug + Send + Sync + 'static> = SessionHandler<
    Transporter<S>,
    SyncSocketHandler<S>, 
    S
    >;
pub type AsyncSessionHandler<S: Sized + Debug + Send + Sync + 'static> = SessionHandler<
    Transporter<S>,
    AsyncSocketHandler<S>, 
    S
    >;
pub type AnySessionHandler<T, R, S: Sized + Debug + Send + Sync + 'static> = SessionHandler<
    T,
    R, 
    S
    >;

///会话句柄
pub struct SessionHandler<T, R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate>  where T: SendHandler<SessionTransport<S>>, R: RecvHandler<SocketMessage<S>>{
    session_id: usize,
    handler: SocketHandler<T, R, SessionTransport<S>, SocketMessage<S>>,
    //waker: Waker,
}
impl<T,R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> Debug for SessionHandler<T, R, S> where T: SendHandler<SessionTransport<S>>, R: RecvHandler<SocketMessage<S>>{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandler")
        .field("session_id", &self.session_id)
        //.field("waker", &self.waker)
        .finish()
    }
}
impl<T, R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate>  SessionHandler<T, R, S>  where T: SendHandler<SessionTransport<S>>, R: RecvHandler<SocketMessage<S>> {
    pub fn new(
        session_id: usize, 
        handler: SocketHandler<T,R,SessionTransport<S>,SocketMessage<S>> ,
        //waker: Waker
    ) -> Self{
        Self{
            session_id, 
            handler, 
            //waker
        }
    }
    ///构造一个仅用于消息发送的Handler
    pub fn msg_handler(&self) -> MsgSendHandler<T, S>{
        MsgSendHandler{
            session_id: self.session_id,
            msg_sender: self.handler.sender.clone(), 
            p: std::marker::PhantomData,
        }
    }
    #[inline]
    pub fn id(&self) -> usize { self.session_id }
    ///```
    /// //try to receive msg from session
    /// ```
    #[inline]
    pub fn try_recv(&mut self) -> Result<SocketMessage<S>, ()> {
        self.handler.try_recv()
    }
    #[inline]
    pub async fn recv_async(&mut self) -> anyhow::Result<SocketMessage<S>>{
        self.handler.recv_async().await
    }
    //异步和同步发送方式都是同步发送,只用同步发送即可
    #[inline]
    pub fn send(&self, transport: SessionTransport<S>) -> anyhow::Result<()>{
        let ret =self.handler.send(transport);
        // if let Err(_) = &ret{
        //     info!("session {} send packet fail. connection maybe reset",self.session_id);
        // }
        //self.waker.wake_by_ref();
        ret?;
        Ok(())
    }
    // #[inline]
    // pub async fn send_async(&mut self, msg: SessionTransport<S>)-> anyhow::Result<()>{
    //     self.handler.send_async(msg).await
    // }
}
#[derive(Clone, Debug)]
pub struct MsgSendHandler<T, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> where T: SendHandler<SessionTransport<S>>{
    session_id: usize,
    ///消息发送句柄
    msg_sender: T, 
    p: std::marker::PhantomData<S>,
}
impl<T, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> MsgSendHandler<T, S> where T: SendHandler<SessionTransport<S>>{
    ///异步和同步均用此方法
    #[inline]
    pub fn send(&self, transport: SessionTransport<S>) -> Result<(),()>{
        let ret =self.msg_sender.send_sync(transport);
        if let Err(_) = &ret{
            info!("session {} send packet fail!, session maybe disconnected",self.session_id);
        }
        //self.waker.wake_by_ref();
        ret.map_err(|_| ())
    }
    #[inline]
    pub fn id(&self) -> usize { self.session_id }
}