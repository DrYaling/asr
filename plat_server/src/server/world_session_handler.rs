use shared::proto::PackBuffer;
use shared::{SyncSessionHandler, SessionTransport, SocketMessage};
#[derive(Debug)]
pub enum WorldSessionHandlerEvent{
    Disconnected,
    OnMessage(PackBuffer)
}
pub struct WorldSessionHandler{
    session_handler: SyncSessionHandler<()>,
}
impl WorldSessionHandler {
    pub fn new(session_handler: SyncSessionHandler<()>) -> Self {
        Self{session_handler}
    }
    #[inline]
    pub fn try_recv(&mut self) -> Result<SocketMessage<()>, ()> {
        self.session_handler.try_recv()
    }
    #[inline]
    pub fn send(&self, transport: SessionTransport<()>) -> Result<(),()>{
        info!("session send packet: {:?}", transport);
        self.session_handler.send(transport).map_err(|_| ())
    }
    ///处理消息,如果没有消息可以处理返回false
    pub fn deal_msg(&mut self) -> Option<WorldSessionHandlerEvent>{
        if let Ok(msg) = self.try_recv(){   
            match msg{
                SocketMessage::Message(boxed_msg) => {                
                    (WorldSessionHandlerEvent::OnMessage(boxed_msg)).into()
                },
                SocketMessage::OnDisconnect => {
                    (WorldSessionHandlerEvent::Disconnected).into()
                }
                _ => None,
            }
        }
        else{
            None
        }
    }
}