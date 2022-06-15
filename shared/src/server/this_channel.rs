//! 当前服务channel
//! 
use std::{collections::BTreeMap, fmt::Debug, marker::PhantomData};
use crate::{AsyncSessionHandler, SessionTransport, SyncSessionHandler, Transporter, server::{channel::*, session::SessionTransportType}};

use super::session::TransferTemplate;

pub struct ThisChannel<T, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> where T: ServiceChannel<S>{
    session_map: BTreeMap<usize, T>,
    queued_sessions: BTreeMap<usize, T>,
    state: ChannelServiceState,
    new_session_handler: crossbeam::channel::Receiver<SyncSessionHandler<S>>,
}
#[allow(unused)]
impl<T, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> ThisChannel<T, S> where T: ServiceChannel<S>{
    pub fn new(rx: crossbeam::channel::Receiver<SyncSessionHandler<S>>) -> Self{
        Self{
            session_map: Default::default(),
            queued_sessions: Default::default(),
            state: ChannelServiceState::Normal,
            new_session_handler: rx,
        }
    }
    pub fn update(&mut self, diff: i64){
        self.handle_event();
        self.session_map.iter_mut().for_each(|(_,session)| session.update(diff));
        self.session_map.retain(|_,session| session.state() != ChannelState::Disconnected);
        self.queued_sessions.iter_mut().for_each(|(_,session)| session.update(diff));
        let connected_sessions = self.queued_sessions.iter().filter(|(_,session)| session.state() == ChannelState::Connected).map(|t| *t.0).collect::<Vec<_>>();
        //已连接session处理
        connected_sessions.into_iter().for_each(|session_id|{
            if let Some(session) = self.queued_sessions.remove(&session_id){
                self.session_map.insert(session_id, session);
            }
        });        
        self.queued_sessions.retain(|_,session| session.state() != ChannelState::Disconnected);
    }
    pub fn close(&mut self){
        self.state = ChannelServiceState::Closed;
        self.session_map.iter_mut().for_each(|(_,session)| {session.disconnect().ok();});
        self.queued_sessions.iter_mut().for_each(|(_,session)| {session.disconnect().ok();});
    }
    ///查询服务状态
    #[allow(unused)]
    #[inline]
    pub fn get_state(&self) -> ChannelServiceState{ self.state }

    fn handle_event(&mut self){
        while let Ok(session) = self.new_session_handler.try_recv() {
            info!("new session {} connected",session.id());
            self.queued_sessions.insert(session.id(), ServiceChannel::new(session, ChannelState::Connecting));
        }
    }
    pub fn send_msg(&self,channel_id: usize, msg: SessionTransport<S>)-> anyhow::Result<()>{
        if let Some(channel) = self.session_map.get(&channel_id) {
            info!("send msg {:?} to channel {}",msg,channel_id);
            channel.send_msg(msg).map_err(|_| crate::error::send_err())?;
        }
        else{
            error!("send msg {:?} to channel {} fail, no sesion found",msg,channel_id);
        }
        Ok(())
    }
}


pub struct AsyncThisChannel<T, E, R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> where T: AsyncServiceChannel<E, R, S> + 'static, E: 'static + Send, R: AsyncServiceDataHandler<E> + std::marker::Send + 'static{
    runtime: tokio::runtime::Runtime,
    state: ChannelServiceState,
    session_map: BTreeMap<usize, Transporter<S>>,
    new_session_handler: tokio::sync::mpsc::UnboundedReceiver<AsyncSessionHandler<S>>,
    phantom: PhantomData<(T, E, R)>,
}
#[allow(unused)]
impl<T, E, R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> AsyncThisChannel<T, E, R, S> where T: AsyncServiceChannel<E,R, S> + 'static, E: 'static + Send, R: AsyncServiceDataHandler<E> + std::marker::Send{
    pub fn new(rx: tokio::sync::mpsc::UnboundedReceiver<AsyncSessionHandler<S>>, worker_count: usize) -> Self{
        Self{
            runtime: tokio::runtime::Builder::new_multi_thread().worker_threads(worker_count).enable_all().build().expect("fail to create async channel service runtime"),
            state: ChannelServiceState::Normal,
            session_map: Default::default(),
            new_session_handler: rx,
            phantom: PhantomData,
        }
    }
    ///关闭服务
    fn close(self){
        self.runtime.shutdown_timeout(std::time::Duration::from_millis(5000));
    }
    ///查询服务状态
    #[allow(unused)]
    #[inline]
    pub fn get_state(&self) -> ChannelServiceState{ self.state }
    ///运行通道服务
    pub fn run(mut self) -> anyhow::Result<tokio::sync::mpsc::UnboundedSender<(usize,SessionTransport<S>)>>{
        let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
        super::worker::RUNTIME.get().expect("worker RUNTIME 未初始化!").spawn(async move{
            if let Err(e) = self.work_runner(rx).await{
                info!("service channel exit for {:?}",e);
            }
        });
        Ok((tx))
    }
    async fn work_runner(&mut self, mut msg_handler: tokio::sync::mpsc::UnboundedReceiver<(usize,SessionTransport<S>)>) -> anyhow::Result<()>{
        while !super::worker::stopped(){
            tokio::select! {
                ret = self.handle_event() =>{
                    ret?;
                },
                msg = msg_handler.recv() => {
                    if let Some((cid, msg))= msg{
                        self.deal_channel_msg(cid, msg).map_err(|e| error!("fail to deal_channel_msg of channel {}, error {:?}",cid,e)).ok();
                    }
                }
            }
        }
        Ok(())
    }
    async fn handle_event(&mut self) -> anyhow::Result<()>{
        if let Some(handler) = self.new_session_handler.recv().await{
            let channel_id = handler.id();
            info!("new channel {} connected",channel_id);
            let mut channel = T::new(handler, ChannelState::Connecting);
            let handler = channel.channel_handler();
            self.session_map.insert(channel_id, handler);
            self.runtime.spawn(async move {
                channel.run().await.map_err(|e| info!("channel {} exit for {:?}",channel_id,e)).ok();
            });
        }
        Ok(())
    }
    ///向会话channel发送消息
    fn deal_channel_msg(&mut self, channel_id: usize, msg: SessionTransport<S>)-> anyhow::Result<()>{
        if let SessionTransportType::Disconnect = msg.transport{
            self.session_map.remove(&channel_id);
        }
        else{
            if let Some(channel) = self.session_map.get(&channel_id) {
                info!("send msg {:?} to channel {}",msg,channel_id);
                channel.send(msg).map_err(|_| crate::error::send_err())?;
            }
            else{
                error!("send msg {:?} to channel {} fail, no sesion found",msg,channel_id);
            }
        }
        Ok(())
    }
}