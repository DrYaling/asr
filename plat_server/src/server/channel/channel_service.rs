use lib::{
    SyncSessionHandler, 
    server::this_channel::ThisChannel,
};
use lib_shared::boxed::MutableBox;
use super::channel_session::ChannelSession;
use once_cell::sync::OnceCell;
static CHANNEL: OnceCell<MutableBox<ThisChannel<ChannelSession, ()>>> = OnceCell::new();
///启动服务channel
pub fn start_up() ->anyhow::Result<()>{
    lib::server::channel::start::<()>(
        &lib::config::get_str("bind_ip").expect("fail"), 
        lib::config::get("channel_port").expect("config channel_port expected")
    )?;
    let (tx,rx) = crossbeam::channel::unbounded();
    lib::server::channel::set_sync_channel_handler(Box::new(move |session: Box<(dyn std::any::Any + std::marker::Send + std::marker::Sync + 'static)>|{
        match session.downcast::<SyncSessionHandler<()>>() {
            Ok(result) => {
                tx.send(*result).ok();
            }
            _ => (),
        }
    }));
    CHANNEL.set(MutableBox::new(ThisChannel::new(rx))).map_err(|_| ()).expect("fail to start plat channel service");
    Ok(())
}
pub fn update(diff: i64){
    let channel = CHANNEL.get().unwrap().get_mut(None);
    let mut channel = match channel{
        None => return error!("fail to get channer service(blocking)"),
        Some(c) => c,
    };
    channel.update(diff);
}