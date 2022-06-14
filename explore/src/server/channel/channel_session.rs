
use lib::{AsyncSessionHandler, AsyncSocketHandler, AsyncSocketSendHandler, SessionTransport, TransportReceiver, Transporter, proto::PackBuffer, server::{channel::{AsyncServiceChannel, AsyncServiceDataHandler, ChannelState, DefaultAsyncServiceDataHandler}}, timer::IntervalTimer};
///本地服务远程会话
pub struct ChannelSession{
    handler: AsyncSessionHandler<()>,
    state: ChannelState,
    heart_timer: IntervalTimer,
    channele_type: lib::proto::ChannelClientType,
    channel_handler: AsyncSocketSendHandler<()>,
    channel_recv: Option<AsyncSocketHandler<()>>, 
    transporter: Option<Transporter<()>>,
    transport_handler: Option<TransportReceiver<()>>
}

impl ChannelSession{
}
pub struct ChannelDataSession{
    transport_handler: TransportReceiver<()>
}
#[async_trait]
impl AsyncServiceDataHandler<SessionTransport<()>> for ChannelDataSession{
    async fn deal(&mut self)-> anyhow::Result<Option<SessionTransport<()>>> {
        Ok(self.transport_handler.recv().await)
    }
}
#[async_trait]
impl AsyncServiceChannel<SessionTransport<()>, ChannelDataSession, ()> for ChannelSession{
    #[inline]
    fn handler_mut(&mut self) -> &mut AsyncSessionHandler<()> {
        &mut self.handler
    }
    #[inline]
    fn handler(&self) -> &AsyncSessionHandler<()> {
        &self.handler
    }
    #[inline]
    fn state(&self) -> ChannelState {
        self.state
    }

    fn new(handler: AsyncSessionHandler<()>, state: ChannelState) -> Self {
        let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
        let (sh,rh) = tokio::sync::mpsc::unbounded_channel();
        Self{
            handler, 
            heart_timer: IntervalTimer::new(30*1000),
            state,
            channele_type: lib::proto::ChannelClientType::UnDefined,
            channel_handler: tx.into(), 
            channel_recv: rx.into(),
            transporter: sh.into(), 
            transport_handler: rh.into(),
        }
    }

    async fn on_packet(&mut self, packet: PackBuffer)-> anyhow::Result<()> {
        let header = packet.header().clone();
        info!("handle explore channel msg {}",header.sub_code());
        match header.sub_code() as u16{
            //创建探索
            lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_REQ => {
                crate::server::entry::on_channel_msg(crate::server::entry::ServerChannelEvent::ChannelMsg((self.session_id(),packet))).await?;
            },
            opcode => {
                warn!("unexpected opcode {} from channel {:?}",opcode,self.client_type());
            }
        }
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
    fn client_type(&self) -> lib::proto::ChannelClientType { self.channele_type.clone() }
    #[inline]
    fn set_client_type(&mut self, ct: lib::proto::ChannelClientType) {
        self.channele_type = ct;
    }

    async fn deal_msg(&mut self, msg: SessionTransport<()>) -> anyhow::Result<()> {
        self.handler.send(msg)?;
        Ok(())
    }

    fn channel_handler(&mut self) -> Transporter<()> {
        self.transporter.take().expect("channel_handler taken")
    }

    fn splite_channel_handler(&mut self)-> AsyncSocketHandler<()> {
        self.channel_recv.take().expect("splite_channel_handler fail, can not splite twice")
    }

    async fn deal_channel_msg(&mut self, msg: lib::SocketMessage<()>) -> anyhow::Result<()> {
        warn!("deal_channel_msg {:?} ignored",msg);
        Ok(())
    }

    async fn deal_teamplate_msg(&mut self, msg: ()) -> anyhow::Result<()> {
        Ok(())
    }
    fn split_data_handler(&mut self) -> Option<ChannelDataSession> {
        ChannelDataSession{
            transport_handler: self.transport_handler.take().expect("split_data_handler fail")
        }.into()
    }

    fn on_close(&mut self) {
        super::channel_service::send_msg(SessionTransport::disconnect(), self.session_id()).map_err(|e| logthrow!(e,e)).ok();
    }
}