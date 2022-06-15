//! 服务间通讯channel
use std::{collections::BTreeMap, fmt::Debug};
use once_cell::sync::OnceCell;
use tokio::net::{TcpListener, TcpStream};

use crate::{AsyncSessionHandler, AsyncSocketHandler, Transporter, timer::IntervalTimer};

use super::{handler::{SyncSessionHandler}, session::{SessionTransport, SocketMessage, TransferTemplate}, worker::{SyncCallback, async_worker::AsyncSessionAddCallback}};
use super::worker::{RUNTIME, SyncSessionAddCallback};
#[derive(Debug, Copy, Clone, PartialEq,Eq)]
pub enum ChannelState{
    Connecting,
    Connected,
    Disconnected,
    Reconnecting
}
#[derive(Debug, Copy, Clone, PartialEq,Eq)]
pub enum ChannelServiceState{
    Normal,
    Closed,
}
///```
/// //服务channel
/// ```
pub trait ServiceChannel<S: Sized + Debug + Send + Sync + 'static + TransferTemplate>{
    fn new(handler: SyncSessionHandler<S>, state: ChannelState) -> Self;
    fn handler_mut(&mut self) -> &mut SyncSessionHandler<S>;
    fn handler(&self) -> &SyncSessionHandler<S>;
    #[inline]
    fn session_id(&self) -> usize{ self.handler().id() }
    fn client_type(&self) -> crate::proto::ChannelClientType;
    fn set_client_type(&mut self, _ct: crate::proto::ChannelClientType) {}
    ///```
    /// //unblock 接收消息
    /// ```
    #[inline]
    fn try_recv(&mut self) -> Result<SocketMessage<S>,()>{
        self.handler_mut().try_recv()
    }
    ///```
    /// //block send msg
    /// ```
    #[inline]
    fn send_msg(&self, msg: SessionTransport<S>)-> anyhow::Result<()>{
        self.handler().send(msg)
    }
    ///断开连接
    #[inline]
    fn disconnect(&mut self) -> anyhow::Result<()>{
        self.set_state(ChannelState::Disconnected);
        self.send_msg(SessionTransport::disconnect())
    } 
    ///发送心跳包
    #[inline]
    fn heartbeat(&self) -> anyhow::Result<()>{
        self.send_msg(SessionTransport::heartbeat())
    }
    fn heart_timer(&mut self) -> &mut IntervalTimer;
    ///channel状态
    fn state(&self) -> ChannelState;
    fn set_state(&mut self, s: ChannelState);
    fn update(&mut self, diff: i64);
    fn handle_event(&mut self){
        while let Ok(msg) = self.try_recv(){
            match msg{
                crate::SocketMessage::Message(packet) => {
                    let code = packet.header().code as u16;
                    if code != crate::proto::proto_code::HEART {
                        info!("handle channel msg {}",code);
                    }
                    if crate::proto::proto_code::HEART == code{
                        self.heart_timer().reset();                        
                    }
                    //握手协议
                    else if crate::proto::proto_code::MSG_CHANNEL_CONNECT == code{
                        match packet.unpack::<crate::proto::MsgChannelConnect>(){
                            Err(_) => {
                                error!("收到握手协议数据错误,连接失败.");
                            },
                            Ok(pack) => {
                                self.heart_timer().reset();
                                self.set_state(ChannelState::Connected);
                                self.set_client_type(pack.get_client_type());
                                info!("收到服务握手协议,服务[{:?}]连接成功",pack.get_client_type());
                            }
                        }
                    }
                    else{
                        if let Err(e) = self.on_packet(packet){
                            info!("service channel {} disconnected for handle packet {} fail {:?}",self.handler().id(),code,e);
                            self.disconnect().ok();
                        }
                        else{
                            self.heart_timer().reset();
                        }
                    }
                },
                crate::SocketMessage::OnDisconnect => {
                    self.set_state(ChannelState::Disconnected);
                },
                SocketMessage::Template(msg) => {
                    self.deal_teamplate_msg(msg).map_err(|e| logthrow!(e,e)).ok();
                }
                _ => (),
            }
        }
    }
    ///收到远端服务包回调
    fn on_packet(&mut self, packet: crate::proto::PackBuffer)-> anyhow::Result<()>;
    ///服务握手协议
    fn hand_shake(&self) -> anyhow::Result<()>{
        let mut pack = crate::proto::MsgChannelConnect::new();
        pack.set_client_id(1);
        pack.set_client_type(self.client_type());
        self.handler().send(SessionTransport::new(
            crate::proto::proto_code::DEFAULT_MAIN_CODE,
            crate::proto::proto_code::MSG_CHANNEL_CONNECT, 
            0, 
            Box::new(pack))).map_err(|_| crate::error::send_err())?;
        Ok(())
    }
    fn deal_teamplate_msg(&mut self, _: S)-> anyhow::Result<()> { Ok(()) }
}
///异步服务数据处理模块
#[async_trait]
pub trait AsyncServiceDataHandler<T> where T: 'static + Send{
    async fn deal(&mut self)-> anyhow::Result<Option<T>>;
}
pub struct DefaultAsyncServiceDataHandler;
#[async_trait]
impl AsyncServiceDataHandler<()> for DefaultAsyncServiceDataHandler{
    async fn deal(&mut self)-> anyhow::Result<Option<()>> {
        Ok(None)
    }
}
pub struct AsyncRpcPlugin<S: Sized + Debug + Send + Sync + 'static + TransferTemplate>{
    pub(crate) requests: BTreeMap<u32,tokio::sync::oneshot::Sender<SocketMessage<S>>>,
    squence: u32,
}
impl<S: Sized + Debug + Send + Sync + 'static + TransferTemplate> AsyncRpcPlugin<S>{
    pub fn new() -> Self{
        AsyncRpcPlugin{ 
            requests: Default::default(),
            squence: 0,
        }
    }
    pub(crate) fn send_rpc(&mut self, mut msg: SessionTransport<S>, timeout: Option<u64>, handler: &mut AsyncSessionHandler<S>) -> anyhow::Result<(u32,impl std::future::Future<Output = anyhow::Result<SocketMessage<S>>>)>{
        self.squence += 1;
        msg.rpc_squence = self.squence;
        let (tx,rx) = tokio::sync::oneshot::channel();
        self.requests.insert(self.squence, tx);
        let fut = async move{
            match timeout{
                Some(tm) if tm > 0 => {
                    let resp = match tokio::time::timeout(std::time::Duration::from_millis(tm), rx).await{
                        Ok(resp) => resp?,
                        Err(_) => crate::error::any_err(std::io::ErrorKind::ConnectionReset)?
                    };
                    Ok(resp)
                },
                _ => {
                    let r = rx.await?;
                    Ok(r)
                }
            }
        };
        handler.send(msg)?;
        Ok((self.squence, fut))
    }
    pub(crate) fn get_rpc_request(&mut self, rpc: u32) -> Option<tokio::sync::oneshot::Sender<SocketMessage<S>>>{
        self.requests.remove(&rpc)
    }
}
#[async_trait]
pub trait AsyncServiceChannel<T, H, S: Sized + Debug + Send + Sync + 'static + TransferTemplate>: Sized + Send where T: 'static + Send, H: AsyncServiceDataHandler<T> + std::marker::Send{
    fn new(handler: AsyncSessionHandler<S>, state: ChannelState) -> Self;
    ///用于服务和每个通道通讯的Handler
    fn channel_handler(&mut self) -> Transporter<S>;
    fn handler_mut(&mut self) -> &mut AsyncSessionHandler<S>;
    fn handler(&self) -> &AsyncSessionHandler<S>;
    ///获取rpc Plugin
    fn get_plugin(&mut self) -> Option<(&mut AsyncRpcPlugin<S>, &mut AsyncSessionHandler<S>)> { None }
    fn get_plugin_mut(&self) -> Option<&mut AsyncRpcPlugin<S>> { None }
    ///open rpc plugin
    #[inline]
    fn open_rpc(&mut self) -> anyhow::Result<()> { 
        crate::error::any_err(std::io::ErrorKind::PermissionDenied)
    }
    #[inline]
    ///rpc 超时时间
    fn rpc_timeout(&self) -> Option<u64> { None }
    ///发送rpc请求
    async fn rpc_request(&mut self, msg: SessionTransport<S>) -> anyhow::Result<SocketMessage<S>> { 
        let tm = self.rpc_timeout();
        let session_id = self.session_id();
        if let Some((plugin, handler)) = self.get_plugin(){
            let (rpc, ret) = plugin.send_rpc(msg, tm, handler)?;
            match ret.await{
                Ok(resp) => Ok(resp),
                Err(e) => {
                    error!("channel {} fail to recv rpc packet {}", session_id, rpc);
                    plugin.get_rpc_request(rpc);
                    let r = Err(e)?;
                    Ok(r)
                },
            }
        }
        else{
            error!("rpc plugin not active!");
            let er = crate::error::send_err_result()?;
            Ok(er)
        }
    }
    #[inline]
    fn session_id(&self) -> usize{ self.handler().id() }
    fn client_type(&self) -> crate::proto::ChannelClientType;
    fn set_client_type(&mut self, _ct: crate::proto::ChannelClientType) {}
    ///分离数据处理模块
    fn split_data_handler(&mut self) -> Option<H> { None }
    fn splite_channel_handler(&mut self)-> AsyncSocketHandler<S>;
    ///```
    /// //unblock 接收消息
    /// ```
    #[inline]
    async fn recv(&mut self) -> anyhow::Result<SocketMessage<S>>{
        self.handler_mut().recv_async().await
    }
    ///```
    /// //block send msg
    /// ```
    #[inline]
    fn send_msg(&self, msg: SessionTransport<S>)-> anyhow::Result<()>{
        self.handler().send(msg)
    }
    ///断开连接
    #[inline]
    fn disconnect(&mut self) -> anyhow::Result<()>{
        self.set_state(ChannelState::Disconnected);
        self.send_msg(SessionTransport::disconnect())
    } 
    fn on_close(&mut self);
    ///发送心跳包
    #[inline]
    async fn heartbeat(&self) -> anyhow::Result<()>{
        self.send_msg(SessionTransport::heartbeat())
    }
    fn heart_timer(&mut self) -> &mut IntervalTimer;
    ///channel状态
    fn state(&self) -> ChannelState;
    fn set_state(&mut self, s: ChannelState);
    async fn handle_event(&mut self) -> anyhow::Result<()>{
        match self.recv().await? {
            crate::SocketMessage::Message(packet) => {
                let code = packet.header().sub_code();
                info!("code is {}.", code);
                if code != crate::proto::proto_code::HEART {
                    info!("handle channel msg {}",code);
                }
                if crate::proto::proto_code::HEART == code{
                    self.heart_timer().reset();                        
                }
                //握手协议
                else if crate::proto::proto_code::MSG_CHANNEL_CONNECT == code{
                    match packet.unpack::<crate::proto::MsgChannelConnect>(){
                        Err(_) => {
                            error!("收到握手协议数据错误,连接失败.");
                        },
                        Ok(pack) => {
                            self.heart_timer().reset();
                            self.set_state(ChannelState::Connected);
                            self.set_client_type(pack.get_client_type());
                            info!("收到服务握手协议,服务[{:?}]连接成功",pack.get_client_type());
                        }
                    }
                }
                else{
                    //rpc 包
                    if packet.header().squence() > 0{
                        if let Some(plg) = self.get_plugin_mut(){
                            if let Some(tx) = plg.get_rpc_request(packet.header().squence()){
                                tx.send(SocketMessage::Message(packet)).map_err(|e| error!("fail to send rpc packet {:?}",e)).ok();
                            }
                            else{
                                error!("no rpc request saved for rpc {}",packet.header().squence());
                            }
                        }
                        else{
                            error!("rpc plugin not active");
                        }
                    }
                    else if let Err(e) = self.on_packet(packet).await{
                        info!("service channel {} disconnected for handle packet {} fail {:?}",self.handler().id(),code,e);
                        self.disconnect().ok();
                    }
                    else{
                        self.heart_timer().reset();
                    }
                }
            },
            crate::SocketMessage::OnDisconnect => {
                self.set_state(ChannelState::Disconnected);
            },
            SocketMessage::Template(msg) => self.deal_teamplate_msg(msg).await?,
            SocketMessage::ChannelMessage(_) => (),
            SocketMessage::SessionMessage(_) => error!("unsupported session message yet"),
        }
        Ok(())
    }
    async fn deal_teamplate_msg(&mut self, msg: S) -> anyhow::Result<()>;
    ///收到远端服务包回调
    async fn on_packet(&mut self, packet: crate::proto::PackBuffer)-> anyhow::Result<()>;
    ///服务握手协议
    fn hand_shake(&self) -> anyhow::Result<()>{
        let mut pack = crate::proto::MsgChannelConnect::new();
        pack.set_client_id(1);
        pack.set_client_type(self.client_type());
        self.handler().send(SessionTransport::new(
            crate::proto::proto_code::DEFAULT_MAIN_CODE,
            crate::proto::proto_code::MSG_CHANNEL_CONNECT, 
            0, 
            Box::new(pack))).map_err(|_| crate::error::send_err())?;
        Ok(())
    }
    async fn deal_msg(&mut self, msg: T) -> anyhow::Result<()>;
    async fn deal_channel_msg(&mut self, msg: SocketMessage<S>) -> anyhow::Result<()>;
    ///运行异步服务
    async fn run(mut self) -> anyhow::Result<()>{
        let mut channel_handler = self.splite_channel_handler();
        trace!("run async channel {}", self.session_id());
        if let Err(e) = self.worker(&mut channel_handler).await{
            info!("channel {} closed {:?}",self.session_id(),e);
            self.on_close();
        }
        Ok(())
    }
    async fn worker(&mut self, channel_handler: &mut tokio::sync::mpsc::UnboundedReceiver<SocketMessage<S>>) -> anyhow::Result<()>{
        if let Some(mut data_handler) = self.split_data_handler(){
            async_loop_with_data_handler(self, &mut data_handler, channel_handler).await?;
        }
        else{
            while !super::worker::stopped() {
                tokio::select! {
                    sr = self.handle_event() => {
                        sr?;
                    },
                    cr = channel_handler.recv() => {
                        if let Some(msg) = cr{
                            self.deal_channel_msg(msg).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

}
async fn async_loop_with_data_handler<T, E, R, S: Sized + Debug + Send + Sync + 'static + TransferTemplate>(service: &mut T, data_handler: &mut R, channel_handler: &mut tokio::sync::mpsc::UnboundedReceiver<SocketMessage<S>>) -> anyhow::Result<()>
where T: AsyncServiceChannel<E, R, S> + std::marker::Send , E: 'static + Send, R: AsyncServiceDataHandler<E> + std::marker::Send{
    while !super::worker::stopped() {
        tokio::select! {
            sr = service.handle_event() => {
                sr?;
            },
            dr = data_handler.deal() => {
                let msg = dr?;
                if let Some(msg) = msg{
                    service.deal_msg(msg).await?;
                }
            }
            cr = channel_handler.recv() => {
                if let Some(msg) = cr{
                    service.deal_channel_msg(msg).await?;
                }
            }
        }
    }
    Ok(())
}
///```
/// //连接远端服务
/// ```
pub fn connect<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: String) -> anyhow::Result<SyncSessionHandler<S>> {
    let (tx,rx) = crossbeam::channel::bounded(1);
    super::worker::RUNTIME.get().unwrap().spawn(async move{
        connect_channel(&address, tx).await.map_err(|e| info!("channel to {} disconnected {:?}",address, e)).ok();
    });
    Ok(rx.recv()?)
}
async fn connect_channel<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: &str, success_handler: crossbeam::channel::Sender<SyncSessionHandler<S>>) -> anyhow::Result<()>{
    let tcp = TcpStream::connect(address).await?;
    let addr = tcp.peer_addr()?;
    let (tx,rx) = crossbeam::channel::bounded(1);
    let mut session = super::session::Session::<_,_,S>::new(
        1, 
        tcp, 
        addr, 
        super::worker::get_shutdown_handler(),
        rx,tx
    );
    success_handler.send(session.get_handler().await)?;
    session.run(false).await?;
    Ok(())
}
///```
/// //连接远端服务
/// ```
pub async fn connect_async<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: String) -> anyhow::Result<SyncSessionHandler<S>>{
    let (tx,mut rx) = tokio::sync::mpsc::channel(1);
    super::worker::RUNTIME.get().unwrap().spawn(async move{
        connect_channel_sync(&address, tx).await.map_err(|e| info!("channel to {} disconnected {:?}",address, e)).ok();
    });
    let ret = match rx.recv().await {
        Some(h) => Ok(h),
        None => Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
    }?;
    Ok(ret)
}
async fn connect_channel_sync<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: &str, success_handler: tokio::sync::mpsc::Sender<SyncSessionHandler<S>>) -> anyhow::Result<()>{
    let tcp = TcpStream::connect(address).await?;
    let addr = tcp.peer_addr()?;
    let (tx,rx) = crossbeam::channel::unbounded();
    let mut session = super::session::Session::<_,_,S>::new(
        1,
         tcp,
          addr,
           super::worker::get_shutdown_handler(),
           rx,tx
        );
    success_handler.send(session.get_handler().await).await.map_err(|_| std::io::Error::from(std::io::ErrorKind::ConnectionRefused))?;
    session.run(false).await?;
    Ok(())
}
///```
/// //开启channel服务
/// ```
pub fn start<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(bind: &str, port: i32) -> anyhow::Result<()>{
    let addr = format!("{}:{}",bind,port);
    RUNTIME.get().unwrap().spawn(async move{
        start_channel::<S>(&addr).await.map_err(|e| info!("channel service {} closed {:?}",addr,e)).ok();
    });
    Ok(())
}
async fn start_channel<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(addr: &str) -> std::io::Result<()>{
    info!("start channel service addr {}",addr);
    let listener = TcpListener::bind(&addr).await?;
    let mut session_id: usize = 1;
    while !super::worker::stopped() {
        let (socket,addr) = listener.accept().await?;
        let (tx,rx) = crossbeam::channel::unbounded();
        let session = super::session::Session::<_,_, S>::new(
            session_id,
             socket,
              addr,
               super::worker::get_shutdown_handler(),
               rx, tx
            );
        RUNTIME.get().unwrap().spawn(async move{
            let mut session = session;
            let handler = session.get_handler().await;
            on_session_accepted(handler);
            session.run(false).await
        });
        session_id += 1;
    }
    std::io::Result::Ok(())
}
static ADD_SYNC_SESSION_HANDLER: OnceCell<SyncCallback> = OnceCell::new();
///设置channel 连接回调
pub fn set_sync_channel_handler(callback: SyncSessionAddCallback){
    ADD_SYNC_SESSION_HANDLER.set(SyncCallback{cb: callback}).map_err(|_| error!("session handler already set")).ok();
}
pub(crate) fn on_session_accepted<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(session: super::channel::SyncSessionHandler<S>){
    ADD_SYNC_SESSION_HANDLER.get().expect("session handler for this worker not set").cb.call_box(Box::new(session));
}
///异步channel模块
pub mod async_channel{
    use crate::server::{handler::AsyncSessionHandler, session::Session, worker::{self, async_worker::AsyncCallback}};
    use super::*;
    ///```
    /// //开启channel服务
    /// ```
    pub fn start<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(bind: &str, port: i32) -> anyhow::Result<()>{
        let addr = format!("{}:{}",bind,port);
        RUNTIME.get().unwrap().spawn(async move{
            start_async_channel::<S>(&addr).await.map_err(|e| error!("channel service {} closed {:?}",addr,e)).and_then(|_| Ok(info!("channel service {} closed",addr))).ok();
        });
        Ok(())
    }
    async fn start_async_channel<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(addr: &str) -> std::io::Result<()>{
        info!("start channel service addr {}",addr);
        let listener = TcpListener::bind(&addr).await?;
        let mut session_id: usize = 1;
        while !worker::stopped() {
            let (socket,addr) = listener.accept().await?;
            let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
            let session = Session::<_,_,S>::new(
                session_id,
                 socket,
                  addr,
                   worker::get_shutdown_handler(),
                   rx, tx
                );
            RUNTIME.get().unwrap().spawn(async move{
                let mut session = session;
                let handler = session.get_handler().await;
                on_async_session_accepted(handler);
                session.run(false).await
            });
            session_id += 1;
        }
        std::io::Result::Ok(())
    }
    ///```
    /// //连接远端服务
    /// ```
    pub async fn connect_async<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: String) -> anyhow::Result<AsyncSessionHandler<S>>{
        let (tx,mut rx) = tokio::sync::mpsc::channel(1);
        worker::RUNTIME.get().unwrap().spawn(async move{
            connect_channel_async::<S>(&address, tx).await.map_err(|e| info!("channel to {} disconnected {:?}",address, e)).ok();
        });
        let ret = match rx.recv().await {
            Some(h) => Ok(h),
            None => Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        }?;
        Ok(ret)
    }
    async fn connect_channel_async<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: &str, success_handler: tokio::sync::mpsc::Sender<AsyncSessionHandler<S>>) -> anyhow::Result<()>{
        let tcp = TcpStream::connect(address).await?;
        let addr = tcp.peer_addr()?;
        let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
        let mut session = Session::<_,_,S>::new(
            1, 
            tcp, 
            addr, 
            super::super::worker::get_shutdown_handler(),
            rx,tx
        );
        success_handler.send(session.get_handler().await).await.map_err(|_| std::io::Error::from(std::io::ErrorKind::ConnectionRefused))?;
        session.run(false).await?;
        Ok(())
    }
    static ADD_ASYNC_SESSION_HANDLER: OnceCell<AsyncCallback> = OnceCell::new();
    ///设置channel 连接回调
    pub fn set_async_channel_handler(callback: AsyncSessionAddCallback){
        ADD_ASYNC_SESSION_HANDLER.set(AsyncCallback{cb: callback}).map_err(|_| error!("session handler already set")).ok();
    }
    pub(crate) fn on_async_session_accepted<S: Sized + Debug + Send + Sync + 'static + TransferTemplate>(session: AsyncSessionHandler<S>){
        ADD_ASYNC_SESSION_HANDLER.get().expect("session handler for this worker not set").cb.call_box(Box::new(session));
    }
}
pub struct AsyncTransportChannel<T, E>{
    sender: tokio::sync::mpsc::UnboundedSender<T>,
    receiver: Option<tokio::sync::mpsc::UnboundedReceiver<E>>,
}
impl<T, E> AsyncTransportChannel<T,E>{
    pub(crate) fn new(t: tokio::sync::mpsc::UnboundedSender<T>, e: tokio::sync::mpsc::UnboundedReceiver<E>) -> Self{
        Self{
            sender: t,
            receiver: e.into(),
        }
    }
    pub fn send(&mut self, msg: T) -> anyhow::Result<Option<tokio::sync::mpsc::UnboundedReceiver<E>>>{
        self.sender.send(msg).map_err(|_| crate::error::send_err())?;
        Ok(self.receiver.take())
    }
    pub fn send_ref(&self, msg: T) -> anyhow::Result<()>{
        self.sender.send(msg).map_err(|_| crate::error::send_err())?;
        Ok(())
    }
    pub fn set_receiver(&mut self, e: tokio::sync::mpsc::UnboundedReceiver<E>){
        self.receiver = e.into();
    }
}
///get unbunded async transport
pub fn transport<T,E>() -> (AsyncTransportChannel<T,E>,AsyncTransportChannel<E,T>){
    let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
    let (sh, rh) = tokio::sync::mpsc::unbounded_channel();
    (
        AsyncTransportChannel::new(tx,rh),
        AsyncTransportChannel::new(sh, rx)
    )
}