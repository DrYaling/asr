//! server
//! 
use tokio::sync::mpsc::{UnboundedReceiver};
use crate::proto::{self, Header};
use super::{handler::SessionHandler, socket_handler::{RecvHandler, SendHandler, SocketHandler}, worker::ServiceCommand};
//use super::waker::Waker;
use std::{fmt::Debug, sync::{atomic::{AtomicI32, Ordering}}};
use protobuf::Message;
use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufWriter}, net::TcpStream};
pub trait TransferTemplate{
    ///get channel proxy from template
    fn get_proxy(&mut self) -> Option<tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>> { None }
}
impl TransferTemplate for () {}
pub enum SessionTransportType<T: Sized + TransferTemplate>{
    Packet(Box<dyn Message>),
    SessionPacket((u64, Box<dyn Message>)),
    Disconnect,
    HeartBeat,
    ///Template is something  you can use at data transfer or channel proxy
    Template(T),
}
impl<T: Sized + TransferTemplate> SessionTransportType<T>{
    pub fn into<V: Sized + TransferTemplate>(self) -> anyhow::Result<SessionTransportType<V>>{
        match self{
            SessionTransportType::Packet(pkt) => Ok(SessionTransportType::Packet(pkt)),
            SessionTransportType::SessionPacket(pkt) => Ok(SessionTransportType::SessionPacket(pkt)),
            SessionTransportType::Disconnect => Ok(SessionTransportType::Disconnect),
            SessionTransportType::HeartBeat => Ok(SessionTransportType::HeartBeat),
            SessionTransportType::Template(_) => super::super::error::any_err(std::io::ErrorKind::InvalidData),
        }
    }
}
impl<T: Sized + TransferTemplate> Debug for SessionTransportType<T>{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Packet(arg0) => f.debug_tuple("Packet").field(arg0).finish(),
            Self::Disconnect => write!(f, "Disconnect"),
            Self::HeartBeat => write!(f, "HeartBeat"),
            Self::Template(_) => write!(f, "Template"),
            Self::SessionPacket(_) => write!(f, "SessionPacket"),
        }
    }
}
///会话消息
/// from worker session to socket session
#[derive(Debug)]
pub struct SessionTransport<T: Sized + TransferTemplate>{
    pub code: u16,
    pub sub_code: u16,
    pub rpc_squence: u32,
    pub transport: SessionTransportType<T>,
}
impl<T: Sized + TransferTemplate> SessionTransport<T> {
    #[inline]
    pub fn new(code: u16, sub_code: u16, rpc_squence: u32, pack: Box<dyn Message>) -> Self {
        Self { code, sub_code, rpc_squence, transport: SessionTransportType::Packet(pack) }
    }
    #[inline]
    pub fn with_opcode(code: u16, sub_code: u16, pack: Box<dyn Message>)  -> Self{
        Self::new(code, sub_code, 0, pack)
    }
    #[inline]
    ///断开连接
    pub fn disconnect() -> Self{
        Self{code: 0, sub_code: 0, rpc_squence: 0, transport: SessionTransportType::Disconnect}
    }
    ///发送心跳
    pub fn heartbeat() -> Self{
        let mut heartbeat = crate::proto::HeartBeat::new();
        heartbeat.set_timestamp(crate::get_timestamp_now());
        Self{
            code: 0, 
            sub_code: crate::proto::proto_code::HEART, 
            rpc_squence: 0, 
            transport: SessionTransportType::Packet(Box::new(heartbeat))
        }
    }
    pub fn template(t: T) -> Self{
        Self{code: 0, sub_code: 0, rpc_squence: 0, transport: SessionTransportType::Template(t)}
    }
    pub fn into<V: Sized + TransferTemplate>(self) -> anyhow::Result<SessionTransport<V>>{
        Ok(SessionTransport::<V>{
            code: self.code, 
            sub_code: self.sub_code, 
            rpc_squence: self.rpc_squence, 
            transport: SessionTransportType::into(self.transport)?
        })
    }
}
pub trait HandlerBoxed {
    fn call_box(self, pack: Box<dyn Message>) -> Result<(),std::io::Error>;
}
impl<F: FnOnce(Box<dyn Message>)-> Result<(),std::io::Error>> HandlerBoxed for F{
    fn call_box(self, pack: Box<dyn Message>) -> Result<(),std::io::Error> {
        self(pack)
    }
}
pub type BoxedRpcHandler = Box<dyn HandlerBoxed + Send + Sync>;
///socket message from socket session to worker session
pub enum SocketMessage<T: Sized + TransferTemplate> {
    ///pack msg 
    Message(PackBuffer),
    ///通道消息,参数0位通道session_id
    ChannelMessage((usize, PackBuffer)),
    ///msg from one tcp session
    SessionMessage((usize, PackBuffer)),
    ///disconnect msg
    OnDisconnect,
    ///Template is something  you can use at data transfer or channel proxy
    Template(T),
}
impl<T: TransferTemplate> Debug for  SocketMessage<T>{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(arg0) => f.debug_tuple("Message").field(arg0).finish(),
            Self::OnDisconnect => write!(f, "OnDisconnect"),
            Self::Template(_) => write!(f, "Template"),
            SocketMessage::ChannelMessage(arg0) => f.debug_tuple("ChannelMessage").field(arg0).finish(),
            SocketMessage::SessionMessage(arg0) => f.debug_tuple("SessionMessage").field(arg0).finish(),
        }
    }
}
use crate::{proto::ByteBuffer};

use crate::proto::PackBuffer;
#[allow(unused)]
const SESSION_STATE_NORMAL: i32 = 0;
const SESSION_STATE_CLOSED: i32 = 1;

///server implyment
pub struct Session<T, R, S: Sized + Clone + Debug + 'static + Send + Sync + TransferTemplate>  where T: SendHandler<SocketMessage<S>>, R: RecvHandler<SocketMessage<S>>{
    pub session_id: usize,    
    state: AtomicI32,
    stream: BufWriter<TcpStream>,
    receiver: Option<UnboundedReceiver<SessionTransport<S>>>,
    handler: Option<SocketHandler<tokio::sync::mpsc::UnboundedSender<SessionTransport<S>>, R, SessionTransport<S>, SocketMessage<S>>>,
    ///write buffer
    write_buffer: ByteBuffer,
    buffer: ByteBuffer,
    current_pack: Option<PackBuffer>,
    #[allow(unused)]
    address: std::net::SocketAddr,
    world_broadcaster: Option<tokio::sync::broadcast::Receiver<ServiceCommand>>,
    msg_sender: T,
    ///proxy for sender
    proxy: Option<tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>>,
    packet_count: u64,
}
unsafe impl<T, R, S: Sized + 'static + Send + Sync + Clone + Debug + TransferTemplate> Send for Session<T, R, S> where T: SendHandler<SocketMessage<S>>, R: RecvHandler<SocketMessage<S>>{}
unsafe impl<T, R, S: Sized + 'static + Send + Sync + Clone + Debug + TransferTemplate> Sync for Session<T, R, S> where T: SendHandler<SocketMessage<S>>, R: RecvHandler<SocketMessage<S>>{}
///thread safe socket stream
/// 
/// unmutable and no lock
#[allow(unused)]
impl<T, R, S: Sized + Clone + 'static + Send + Sync + Debug + TransferTemplate> Session<T, R, S> where T: SendHandler<SocketMessage<S>>, R: RecvHandler<SocketMessage<S>>{
    pub(crate) fn new(
        session_id: usize, 
        stream: TcpStream, 
        address: std::net::SocketAddr, 
        world_broadcaster: tokio::sync::broadcast::Receiver<ServiceCommand>,
        rh: R, sh: T
    ) -> Self{
        trace!("new session {}",session_id);
        let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
        let ret = Self{
            state: AtomicI32::new(0),
            session_id,
            stream:  BufWriter::new(stream),
            receiver: rx.into(),
            handler: SocketHandler::new(tx, rh).into(),
            msg_sender: sh,
            write_buffer: ByteBuffer::new(1024),
            buffer: ByteBuffer::new(4 * 1024),
            address, 
            current_pack: None,
            world_broadcaster: world_broadcaster.into(),
            packet_count: 0,
            proxy: None,
        };
        trace!("session {} created",session_id);
        return ret;
    }
    pub async fn get_handler(&mut self) -> SessionHandler<tokio::sync::mpsc::UnboundedSender<SessionTransport<S>>, R, S>{
        SessionHandler::new(
            self.session_id,
            self.handler.take().expect("SocketHandler Not Initialized"),
      //      super::waker::AsyncWaker.await,
        )
    }
    ///运行会话
    pub async fn run(&mut self, connect_timeout_check: bool) -> std::io::Result<()>{
        let mut world_broadcaster = self.world_broadcaster.take().unwrap();
        let result = tokio::select! {
            ret = self.run_session(connect_timeout_check) => {
                ret
            }
            _ = world_broadcaster.recv() =>{
                Ok(())
            }
        };
        self.shutdown().await.ok();
        log_info!("session {} quit, reason {:?}", self.session_id, result);
        result
    }
    async fn run_session(&mut self, connect_timeout_check: bool) -> std::io::Result<()>{
        let mut result = Ok(());
        log_info!("run session {} with connect_timeout_check {}",self.session_id, connect_timeout_check);
        let mut receiver = self.receiver.take().unwrap();
        if let Some(h) = &self.handler{
            trace!("session {} connected with handler {:p}", self.session_id, h);
        }
        //
        //session connect timeout
        const PACKET_TIMEOUT: u64 = 3_000;
        //check timeout first
        while connect_timeout_check{
            match tokio::time::timeout(std::time::Duration::from_millis(PACKET_TIMEOUT), self.read()).await{
                Ok(res) => {
                    //log_info!("session {} connection result {:?}", self.session_id, res);
                    match res{
                        Ok(size) if size == 0 => {
                            self.send_sync(SocketMessage::OnDisconnect).ok();
                            return Ok(());
                        },
                        Err(e) => {
                            log_info!("session {} read socket stream fail, session killed {:?}",self.session_id,e);
                            self.send_sync(SocketMessage::OnDisconnect).ok();
                            return Err(std::io::ErrorKind::ConnectionAborted.into());
                        },
                        _ => (),
                    }
                },
                Err(_) => {
                    //log_info!("session {} connection timeout!", self.session_id);
                    self.send_sync(SocketMessage::OnDisconnect).ok();
                    log_info!("session {} connect timeout packet count {}", self.session_id, self.packet_count);
                    return Err(std::io::ErrorKind::TimedOut.into());
                },
            }
            //if current packet receive success, means this is an valid connection
            //the packet validation will be check in the user part
            if self.current_pack.is_none() || self.packet_count > 0{
                break;
            }
        }
        loop {
            //说明 AsyncRead/WriteExt 和 UnbundedReceiver 是所谓Cancellation safety,所以可以用select宏来处理读写操作
            if let Some(transport) =  tokio::select! {
                res = self.read() => {
                    match res{
                        Ok(size) if size == 0 => {
                            self.send_sync(SocketMessage::OnDisconnect).ok();
                            log_info!("session {} disconnected by remote", self.session_id);
                            break;
                        },
                        Err(e) => {
                            log_info!("session {} read socket stream fail, session killed {:?}",self.session_id,e);
                            self.send_sync(SocketMessage::OnDisconnect).ok();
                            result = Err(std::io::ErrorKind::ConnectionAborted.into());
                            break;
                        }
                        _ => None,
                    }
                },
                other = receiver.recv() => {
                    other
                }
            }{
                if let Err(e) = self.queue_write(transport).await{
                    log_info!("session {} send packets fail {:?}",self.session_id,e);
                    self.send_sync(SocketMessage::OnDisconnect).ok();
                    result = Err(e);
                    break;
                }
            }
        }
        result
    }
    //TODO 由于目前心跳验证未开启,客户端在某些情况下断开会导致服务器无法识别,fd不会释放,导致在linux上epoll_wait浪费大量CPU性能
    async fn read(&mut self)-> std::io::Result<usize>{
        let read_size = self.stream.read_buf(&mut self.buffer).await?;
        if 0 == read_size {
            // The remote closed the connection. For this to be a clean
            // shutdown, there should be no data in the read buffer. If
            // there is, this means that the peer closed the socket while
            // sending a frame.
            if self.buffer.size() == 0 {
                //log_info!("socket {} disconnected",self.session_id);
                return Ok(0);
            } else {
                //log_info!("socket {} disconnected while sending",self.session_id);
                return Err(std::io::ErrorKind::ConnectionReset.into());
            }
        }
        self.buffer.write_complete(read_size);
        // println!("recv buffer size {}, {}",read_size,self.current_pack.is_some());
        loop {
            match self.current_pack.as_mut(){
                Some(pack) => {
                    if pack.read_from_buffer(&mut self.buffer).is_ok(){    
                        pack.crc()?;                                                                                            
                        let pack = self.current_pack.take();     
                        self.packet_count.wrapping_add(1);
                        match self.on_msg(pack.unwrap()) {
                            Ok(_) => (),
                            Err(e) => {
                                if let Some(h) = &self.handler{
                                    log_error!("session {},handler {:p} fail to send pack(Cur) {:?}", self.session_id, h, e);
                                }
                                else{
                                    log_error!("session {},handler None,fail to send pack(Cur) {:?}", self.session_id, e);
                                }
                                return Err(std::io::ErrorKind::InvalidData.into());
                                
                            }
                        }
                        //读取这个包成功，尝试继续读取下一个包
                    }
                    else{
                        //当前包未读取完成，保存这个包继，在下一次event继续读取
                        break;
                    }
                },
                None => {
                    //读取一个包，如果读取失败，则等待下一次读取
                    //println!("buffer size {} - {:?}",byte_buffer.size(),byte_buffer.as_slice());
                    let header_size = Header::header_size(self.buffer.as_slice());
                    if self.buffer.size() >= header_size{
                        let mut pack = proto::PackBuffer::from(&self.buffer.as_slice()[0..header_size]).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                        let code = pack.header().sub_code();
                        let size = pack.header().size();
                        //println!("session {} read packet header size {}, code {}, data size {}",self.session_id,header_size, code, size);
                        if code > 0 && code < 60000 && (size < u32::MAX & 0x00ffffff){
                            self.buffer.read_complete(header_size);
                            if pack.read_from_buffer(&mut self.buffer).is_ok(){  
                                pack.crc()?;                          
                                self.packet_count.wrapping_add(1);
                                match self.on_msg(pack){
                                    Ok(_) => (),
                                    Err(e) => {
                                        if let Some(h) = &self.handler{
                                            log_error!("session {},handler {:p} fail to send pack(Cur) {:?}", self.session_id, h, e);
                                        }
                                        else{
                                            log_error!("session {},handler None,fail to send pack(Cur) {:?}", self.session_id, e);
                                        }
                                        self.send_sync(SocketMessage::OnDisconnect).ok();
                                        return Err(std::io::ErrorKind::InvalidData.into());
                                    }
                                }
                                //读取这个包成功，尝试继续读取下一个包
                            }
                            else{
                                //当前包未读取完成，保存这个包继，在下一次event继续读取
                                self.current_pack = pack.into();
                                break;
                            }
                        }
                        else{
                            log_error!("session {} recv error packet of code {}, data size {}",self.session_id, code, size);
                            return Err(std::io::ErrorKind::InvalidData.into());
                        }
                    }
                    else{
                        break;
                    }
                }
            }
        }
        Ok(read_size)
    }
    fn on_msg(&mut self, pack: PackBuffer) -> anyhow::Result<()>{
        let (code,squence) =( pack.header().sub_code(),pack.header().squence());
        if code != crate::proto::proto_code::HEART {
            if squence > 0{
                log_info!("session {} on msg {} rpc {}",self.session_id,code,squence);
            }
            else{
                log_info!("session {} on msg {}",self.session_id,code);
            }
        }
        self.send_sync(SocketMessage::Message(pack))?;
        Ok(())
    }
    ///receive queue buffer and write to stream
    pub async fn queue_write(&mut self, msg: SessionTransport<S>) -> std::io::Result<usize>{        
        let buffer = &mut self.write_buffer;
        let size = buffer.size();
        let SessionTransport{code,sub_code,rpc_squence,transport} = msg;
        if sub_code as u16 != crate::proto::proto_code::HEART {
            log_info!("session {} send packet {}",self.session_id,code);
        }
        match transport{
            SessionTransportType::Packet(pack) => {
                match crate::proto::pack_box(code, sub_code as u16, pack,if_else!(rpc_squence == 0,None, Some(rpc_squence))){
                    Ok(mut bytes) => {
                        buffer.write(bytes.as_slice(), bytes.len());
                    },
                    Err(_) => {
                        log_error!("session {} fail to write pack: write buffe pack fail!",self.session_id);
                        return Err(std::io::ErrorKind::InvalidData.into());
                    }
                }
            },
            SessionTransportType::Disconnect => {
                return Err(std::io::ErrorKind::ConnectionAborted.into());
            },
            SessionTransportType::HeartBeat => (),
            SessionTransportType::Template(mut msg) => {
                if let Some(proxy) = msg.get_proxy(){
                    self.proxy = proxy.into();
                }
                log_info!("custom msg {:?}",msg);
            },
            SessionTransportType::SessionPacket(_) => todo!(),
        }
        self.flush().await
    }
    fn send_sync(&self, msg: SocketMessage<S>) -> anyhow::Result<()>{
        log_info!("send_sync msg is {:?}", msg);
        match &self.proxy{
            Some(proxy) => {
                match msg{
                    //transfer normal packet into session msg
                    SocketMessage::Message(msg) => proxy.send_sync(SocketMessage::SessionMessage((self.session_id, msg))),
                    SocketMessage::ChannelMessage(t) => proxy.send_sync(SocketMessage::ChannelMessage(t)),
                    SocketMessage::OnDisconnect => proxy.send_sync(SocketMessage::OnDisconnect),
                    SocketMessage::Template(t) => {
                        self.msg_sender.send_sync(SocketMessage::Template(t))
                    },
                    SocketMessage::SessionMessage(t) => proxy.send_sync(SocketMessage::SessionMessage(t)),
                }
            },
            None => self.msg_sender.send_sync(msg),
        }
    }
    pub async fn shutdown(&mut self)-> std::io::Result<()>{
        self.state.store(SESSION_STATE_CLOSED, Ordering::Release);
        self.send_sync(SocketMessage::OnDisconnect).ok();
        self.stream.shutdown().await
    }
    
    ///flush the session, send all the remaning buffers
    pub async fn flush(&mut self) -> std::io::Result<usize>{
        let buffer = &mut self.write_buffer;
        if buffer.size() > 0{
            let write_size = buffer.size();
            //这里不是在tokio::select分支,所以一定能完成整个操作
            self.stream.write_all(buffer.as_slice()).await?;
            self.stream.flush().await?;
            buffer.read_complete(write_size).map_err(|_| logthrow!(Err::<(),&'static str>("buffer read_complete fail!"),std::io::Error::from(std::io::ErrorKind::BrokenPipe)))?;
            buffer.trim_step();
            Ok(write_size)
        }
        else{
            Ok(0)
        }
    }
}
#[cfg(test)]
#[test]
fn test_timeout(){
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(8).enable_all().build().unwrap();
    let (_,rx) = tokio::sync::mpsc::channel::<bool>(1);
    runtime.block_on(async{
        let mut rx = rx;
        match tokio::time::timeout(std::time::Duration::from_millis(3_000), rx.recv()).await{
            Ok(resp) => println!("ok {:?}", resp),
            Err(e) => println!("error {:?}", e),
        }
    });
}