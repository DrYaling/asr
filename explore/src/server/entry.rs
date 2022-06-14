//! 探索入口
//! 
use crate::server::explore::{player_session::PlayerSessionBuilder};

use super::explore::{player_session::PlayerSessionInfo, ExploreSharedChannel};
use once_cell::sync::Lazy;
use lib::{AsyncSessionHandler, SessionTransport, SocketMessage, proto::PackBuffer};
use lib_shared::boxed::MutexArc;
use super::explore::{Explore};
use std::collections::BTreeMap;
///服务channel消息
#[derive(Debug)]
pub enum ServerChannelEvent{
    ///client session_id, packet
    ChannelMsg((usize,PackBuffer)),
}
pub struct ExploreHandler{
    pub player_id: u64,
    pub handler: tokio::sync::mpsc::UnboundedSender<SocketMessage<PlayerSessionInfo>>,
}
///channel map
static CHANNEL_MAP: Lazy<MutexArc<BTreeMap<u64,ExploreHandler>>> = Lazy::new(|| MutexArc::new(BTreeMap::new()));
pub fn on_new_session(session: AsyncSessionHandler<ExploreSharedChannel>){
    let id = session.id();
    trace!("on_new_session {}", id);
    let context = lib::AsyncContext::<super::explore::player_session::PlayerSession,_,ExploreSharedChannel>::new(session.into(), PlayerSessionBuilder);
    context.start().map_err(|e| error!("fail to  start context{}, {:?}",id,e)).ok();
}
pub async fn on_channel_msg(msg: ServerChannelEvent)->anyhow::Result<()>{
    info!("recv channel msg {:?}",msg);
    match msg{
        ServerChannelEvent::ChannelMsg((channel,pack)) => {
            handle_channel_msg(channel, pack).await?;
        },
    }
    Ok(())
}
pub fn remove_explore(player_id: u64){
    info!("explore of player {} closed",player_id);
    let mut channels = CHANNEL_MAP.get_mut(None).unwrap();
    channels.remove(&player_id);
}
pub(crate) async fn bind_explore(player_id: u64, player_info: PlayerSessionInfo) -> anyhow::Result<()>{
    let event_handlers = CHANNEL_MAP.get();
    match event_handlers.get(&player_id){
        Some(handler) => {
            handler.handler.send(SocketMessage::Template(player_info))?;
        },
        None => {
            //没有这个探索,直接断开连接
            player_info.session_handler.send(SessionTransport::disconnect())?;
        }
    }
    Ok(())
}
///处理通道消息
async fn handle_channel_msg(channel_id: usize, packet: PackBuffer)-> anyhow::Result<()>{
    match  packet.header().sub_code() as u16{
        lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_REQ => {
            let lib::proto::Ps2EsMsgExploreReq{
                player_id,
                explore_id, 
                characters,
                gm_authority,
                tasks_id,..
            } = packet.unpack::<lib::proto::Ps2EsMsgExploreReq>().map_err(|_| lib::error::send_err())?;
            {
                if let Some(explore) =  CHANNEL_MAP.get().get(&player_id){
                    info!("explore player {} exist, try connecting...",player_id);
                    explore.handler.send(SocketMessage::ChannelMessage((channel_id,packet)))
                    .map_err(|e| logthrow!(e,"fail to send channel msg",lib::error::send_err()))?;
                    return Ok(());
                }
            }
            log_info!("create explore chapter {}, player {}, tasks {:?}", explore_id, player_id, tasks_id);
            let header = packet.header();
            //创建探索,并加载数据
            let mut explore = Explore::create(
                player_id, 
                explore_id, 
                channel_id, 
                characters, 
                gm_authority,
                tasks_id.first().copied().unwrap_or_default().max(0)
            )?;
            match explore.create_explore(Some(1*60*1000)).await{
                Ok(_) => {
                    let mut resp = lib::proto::Es2PsMsgExploreResp::new();
                    resp.set_result(lib::proto::ExploreCreateResult::SUCCESS);
                    resp.set_explore_uuid(explore.get_explore_uuid());
                    resp.set_player_id(player_id);
                    resp.set_access_token(explore.access_token().to_string());
                    super::channel::channel_service::send_msg(SessionTransport::new(
                        lib::proto::proto_code::DEFAULT_MAIN_CODE,
                        lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_RESP, 
                        header.squence(), 
                        Box::new(resp)), 
                        explore.get_plat_server()).map_err(|e| logthrow!(e,e)).ok();
                },
                Err(e) => {
                    error!("create explore {:?} fail {:?}",(player_id,explore_id),e);
                    let mut resp = lib::proto::Es2PsMsgExploreResp::new();
                    resp.set_result(lib::proto::ExploreCreateResult::FAIL);
                    super::channel::channel_service::send_msg(SessionTransport::new(
                        lib::proto::proto_code::DEFAULT_MAIN_CODE,
                        lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_RESP,
                        header.squence(), 
                          Box::new(resp)), channel_id)?;
                }
            }
            let mut context = lib::AsyncContext::<Explore,_, ()>::from(None, explore);
            let event_handler = context.inner_mut().event_handler();
            CHANNEL_MAP.get_mut(None).unwrap().insert(player_id, ExploreHandler{handler: event_handler, player_id});
            context.start().map_err(|e| error!("fail to  start context{}, {:?}",player_id,e)).ok();

        },
        lib::proto::proto_code::msg_id_es_ps::FIGHT_SUCCESS_RESP => {
            let lib::proto::Ps2EsMsgFightSucessResp{
                player_id,..
            } = packet.unpack::<lib::proto::Ps2EsMsgFightSucessResp>().map_err(|_| lib::error::send_err())?;
            {
                if let Some(explore) =  CHANNEL_MAP.get().get(&player_id){
                    explore.handler.send(SocketMessage::ChannelMessage((channel_id,packet)))
                    .map_err(|e| logthrow!(e,"fail to send channel msg",lib::error::send_err()))?;
                    return Ok(());
                }
            }
        },
        opcode => {
            error!("unrecognized channel opcode {}",opcode);
        },
    }
    Ok(())
}
#[allow(unused)]
pub(crate) fn get_bearing() -> usize {
    CHANNEL_MAP.get_mut(None).unwrap().len()
}