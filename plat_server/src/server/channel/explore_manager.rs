use crossbeam::channel::Sender;
use lib::{SessionTransport, server::channel::{ChannelState, ServiceChannel}};
use once_cell::sync::OnceCell;
use crate::server::world::WorldCommand;
use lib_shared::boxed::MutexArc;
use super::{explore_channel::ExploreChannel};
static EXPLORE_CLIENT: OnceCell<MutexArc<ExploreChannel>> = OnceCell::new();
///启动探索服务通讯通道
pub fn start_up(msg_handler: Sender<WorldCommand>) -> anyhow::Result<()>{
    let channnel = ExploreChannel::start_up(msg_handler,10)?;
    channnel.hand_shake()?;
    EXPLORE_CLIENT.set(MutexArc::new(channnel)).map_err(|_| std::io::Error::from(std::io::ErrorKind::ConnectionAborted))?;
    Ok(())
}
pub fn update(diff: i64){
    let mut explore_channel = EXPLORE_CLIENT.get().unwrap().get_mut(None).unwrap();

    explore_channel.update(diff as i64);
    if explore_channel.state() == ChannelState::Disconnected{
        //reconnect
        explore_channel.reconnect_async().ok();
        //error!("explore_channel disconnected!, reconnect now");
    }
}
//向平台服发送消息
pub fn send_msg(msg: SessionTransport<()>) -> anyhow::Result<()>{
    EXPLORE_CLIENT.get().expect("探索通道未开启").get().send_msg(msg).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
    Ok(())
}