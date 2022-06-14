//!探索服通讯通道

use futures::FutureExt;
use lib::{SyncSessionHandler, server::{channel::{self, ServiceChannel, ChannelState}}, timer::IntervalTimer};

use crate::server::world::WorldCommand;
pub struct ExploreChannel{
    handler: SyncSessionHandler<()>,
    heart_timer: IntervalTimer,
    state: ChannelState,
    reconnect_handler: crossbeam::channel::Receiver<SyncSessionHandler<()>>,
    reconnect_callback: crossbeam::channel::Sender<SyncSessionHandler<()>>,
    msg_handler: crossbeam::channel::Sender<WorldCommand>,
}
impl ExploreChannel{
    ///启动服务
    pub(crate) fn start_up(msg_handler: crossbeam::channel::Sender<WorldCommand>, mut try_times: i32) -> anyhow::Result<Self>{
        let addr = format!("{}:{}",lib::config::get_str("explore_channel_ip").unwrap(),
        lib::config::get::<i32>("explore_channel_port").expect("config explore_channel_port expected"));
        let handler = loop {
            if let Ok(conn) = channel::connect(addr.clone()){
                break conn;
            }
            try_times -= 1;
            if try_times <= 0{
                return lib::error::any_err(std::io::ErrorKind::BrokenPipe);
            }
            warn!("fai to connect to explore channel {}, try after 3 seconds",addr);
            std::thread::sleep(std::time::Duration::from_millis(3000));
        };
        info!("explore_channel connect succes,session id {}",handler.id());
        let (tx,rx) = crossbeam::channel::bounded(1);
        Ok(Self{
            handler, 
            heart_timer: IntervalTimer::new(12*1000),
            state: ChannelState::Connected, 
            reconnect_handler: rx,
            reconnect_callback: tx, 
            msg_handler: msg_handler,
        })
    }
    pub fn reconnect_async(&mut self) -> anyhow::Result<()>{
        let addr = format!("{}:{}",lib::config::get_str("explore_channel_ip").unwrap(),
        lib::config::get::<i32>("explore_channel_port").expect("config explore_channel_port expected"));
        let cb = self.reconnect_callback.clone();
        lib::db::send_query(Box::new(async move {
            while let Err(e) =  Self::reconnect(addr.clone(),cb.clone()).await{
                info!("explore_channel {} reconnect fail!{:?}, try again after 3sec",addr,e);
                tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                info!("explore_channel reconnect fail!, try again");
            }
        }).boxed()).map_err(|e| logthrow!(e,std::io::Error::from(std::io::ErrorKind::BrokenPipe)))?;
        self.state =  ChannelState::Reconnecting;
        Ok(())
    }
    async fn reconnect(addr: String, reconnect_callback:  crossbeam::channel::Sender<SyncSessionHandler<()>>) -> anyhow::Result<()>{
        let handler = channel::connect_async(addr).await?;
        reconnect_callback.send(handler)?;
        Ok(())
    }
    fn handle_reconnect(&mut self){
        if let Ok(handler) = self.reconnect_handler.try_recv(){
            self.handler = handler;
            self.state =  ChannelState::Connected;
            self.hand_shake().map_err(|e| logthrow!(e,"channel reconnect hand_shake fail",())).ok();
        }
    }
    #[inline]
    fn reconnecting(&self) -> bool{
        self.state == ChannelState::Reconnecting
    }

}
impl ServiceChannel<()> for ExploreChannel{
    #[inline]
    fn handler_mut(&mut self) -> &mut SyncSessionHandler<()> {
        &mut self.handler
    }
    #[inline]
    fn handler(&self) -> &SyncSessionHandler<()> {
        &self.handler
    }

    #[inline]
    fn state(&self) -> channel::ChannelState {
        self.state
    }

    fn new(handler: SyncSessionHandler<()>, state: ChannelState) -> Self {
        let (tx,rx) = crossbeam::channel::bounded(1);
        let (tx1,_rx1) = crossbeam::channel::bounded(1);
        Self{
            handler, 
            heart_timer: IntervalTimer::new(12*1000),
            state, 
            reconnect_handler: rx,
            reconnect_callback: tx, 
            msg_handler: tx1,
        }
    }
    fn update(&mut self, diff: i64){
        if self.reconnecting() {
            self.handle_reconnect();
            if self.reconnecting(){
                return;
            }
        }
        self.handle_event();
        self.heart_timer.update(diff);
        if self.heart_timer.passed(){
            self.heart_timer.reset();
            self.heartbeat().ok();
        }
    }

    fn on_packet(&mut self, packet: lib::proto::PackBuffer)-> anyhow::Result<()> {
        self.msg_handler.send(WorldCommand::ExploreMsg(packet))?;
        Ok(())
    }
    #[inline]
    fn heart_timer(&mut self) -> &mut IntervalTimer {
        &mut self.heart_timer
    }

    #[inline]
    fn set_state(&mut self, s: ChannelState) {
        self.state = s;
    }
    #[inline]
    fn client_type(&self) -> lib::proto::ChannelClientType {lib::proto::ChannelClientType::PlatServer }
}