
use shared::{SyncSessionHandler, proto::PackBuffer, server::{channel::{ChannelState, ServiceChannel}}, timer::IntervalTimer};
///本地服务远程会话
pub struct ChannelSession{
    handler: SyncSessionHandler<()>,
    state: ChannelState,
    heart_timer: IntervalTimer,
    channele_type: shared::proto::ChannelClientType,
}

impl ChannelSession{
}
impl ServiceChannel<()> for ChannelSession{
    #[inline]
    fn handler_mut(&mut self) -> &mut SyncSessionHandler<()> {
        &mut self.handler
    }
    #[inline]
    fn handler(&self) -> &SyncSessionHandler<()> {
        &self.handler
    }
    #[inline]
    fn state(&self) -> ChannelState {
        self.state
    }

    fn new(handler: SyncSessionHandler<()>, state: ChannelState) -> Self {
        Self{
            handler, 
            heart_timer: IntervalTimer::new(30*1000),
            state,
            channele_type: shared::proto::ChannelClientType::UnDefined,
        }
    }
    fn update(&mut self, diff: i64){
        self.handle_event();
        self.heart_timer.update(diff);
        if self.heart_timer.passed(){
            self.state = ChannelState::Disconnected;
            self.disconnect().ok();
            info!("service channel {} disconnected",self.handler.id());
        }
    }

    fn on_packet(&mut self, _packet: PackBuffer)-> anyhow::Result<()> {
        //TODO
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
    fn client_type(&self) -> shared::proto::ChannelClientType { self.channele_type.clone() }
    #[inline]
    fn set_client_type(&mut self, ct: shared::proto::ChannelClientType) {
        self.channele_type = ct;
    }
}