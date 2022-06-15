//! 探索玩家临时会话

use shared::{AsyncContextImpl, AsyncSessionHandler, SocketMessage, server::{context::AsyncContextBuilder, session::TransferTemplate}};

use super::ExploreSharedChannel;
///玩家会话连接超时
const SESSION_CONNECT_TIMEOUT: i64 = 1*60*1000;
#[derive(Debug)]
pub struct PlayerSessionInfo{
    pub player_id: u64,
    pub session_handler: AsyncSessionHandler<ExploreSharedChannel>,
    pub token: String,
    pub explore_uuid: u64,
    pub rpc: u32,
}
impl TransferTemplate for PlayerSessionInfo{
    fn get_proxy(&mut self) -> Option<tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>> { 
        None
     }
}
pub struct PlayerSessionBuilder;
impl AsyncContextBuilder for PlayerSessionBuilder{}
///玩家会话,连接成功后合并到Explore中
pub struct PlayerSession{
    time_out: i64,
}
#[async_trait]
impl AsyncContextImpl<PlayerSessionBuilder,ExploreSharedChannel> for PlayerSession{
    fn new(_: PlayerSessionBuilder) -> Self {
        Self{
            time_out: SESSION_CONNECT_TIMEOUT,
        }
    }

    async fn deal_msg(&mut self, msg: shared::SocketMessage<ExploreSharedChannel>, upper_handler: &mut Option<AsyncSessionHandler<ExploreSharedChannel>>) -> anyhow::Result<()> {
        match msg{
            SocketMessage::Message(pack) => {
                let rpc = pack.header().squence();
                match pack.header().sub_code() as u16{
                    crate::msg_id::CREATE_EXPLORE_REQ => {
                        if let Some(handler) = upper_handler.take(){
                            let msg = pack.unpack::<shared::proto::C2EsMsgStartExploreReq>().map_err(|_| shared::error::unpack_err())?;
                            let player_info = PlayerSessionInfo{
                                player_id: msg.player_id,
                                explore_uuid: msg.explore_uuid,
                                rpc,
                                token: msg.access_token,
                                session_handler: handler,
                            };
                            super::super::entry::bind_explore(player_info.player_id,player_info).await?;
                            //返回错误以停止当前会话context
                            return shared::error::any_err(std::io::ErrorKind::ConnectionReset);
                        }

                    },
                    _ => (),
                }
            },
            _ => (),
        }
        Ok(())
    }
    ///如果连接超时,直接断开
    async fn context_check(&mut self, _: &mut Option<AsyncSessionHandler<ExploreSharedChannel>>) -> anyhow::Result<()> {
        tokio::time::timeout(std::time::Duration::from_millis(self.time_out as u64), async{ }).await?;
        Ok(())
    }

    fn on_close(&mut self) {
    }
}