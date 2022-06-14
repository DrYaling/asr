use lib::{AsyncSessionHandler, AsyncSocketHandler, SessionTransport, Transporter, server::{channel::DefaultAsyncServiceDataHandler, this_channel::AsyncThisChannel}};
use super::channel_session::ChannelSession;
use once_cell::sync::OnceCell;
static CHANNEL: OnceCell<tokio::sync::mpsc::UnboundedSender<(usize, SessionTransport<()>)>> = OnceCell::new();
///启动服务channel
pub fn start_up() ->anyhow::Result<()>{
    lib::server::channel::async_channel::start::<()>(
        &lib::config::get_str("bind_ip").expect("fail"), 
        lib::config::get("channel_port").expect("config channel_port expected")
    )?;
    let (tx,rx) = tokio::sync::mpsc::unbounded_channel();
    lib::server::channel::async_channel::set_async_channel_handler(Box::new(move |session: Box<dyn (::std::any::Any) + Send + Sync + 'static>|{
        if let Ok(result) = session.downcast::<AsyncSessionHandler<()>>(){            
            tx.send(*result).ok();
        }
    }));
    let channel = lib::server::this_channel::AsyncThisChannel::<ChannelSession,SessionTransport<()>, super::channel_session::ChannelDataSession, ()>::new(rx, 2);
    let msg_handler = channel.run()?;
    CHANNEL.set(msg_handler).map_err(|_| ()).expect("fail to start plat channel service");
    Ok(())
}
pub fn send_msg(msg: SessionTransport<()>, session_id: usize) -> anyhow::Result<()>{
    info!("send msg {:?} to channel {}",msg,session_id);    
    CHANNEL.get().unwrap().send((session_id, msg))?;
    Ok(())
}