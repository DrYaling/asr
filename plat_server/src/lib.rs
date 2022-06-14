//! this is a demo project of sync mode server
#[allow(unused_imports)]
#[macro_use]
extern crate lib_shared;
#[macro_use]
extern crate log;
extern crate anyhow;
mod server;
mod player;
mod msg_id;
#[allow(unused)]
fn start_up() -> Result<i32,i32>{
    server::start::start().map_err(|e| {error!("fail to start server: {:?}",e);-1})?;
    Ok(0)
}