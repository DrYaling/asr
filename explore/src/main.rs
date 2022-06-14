#[allow(unused_imports)]
#[macro_use]
extern crate lib_shared;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate log;
#[allow(unused)]
#[macro_use]
extern crate anyhow;
mod server;
mod msg_id;
fn main() -> anyhow::Result<()>{
    // let mut req = lib::proto::C2PMsgLogin::new();
    // req.account = "111".to_string();
    // if let Ok(pack) = lib_shared::net_core::pack_box(101, 50032, Box::new(req), Some(0)){
    //     if let Ok(_) = lib_shared::net_core::unpack_header(&pack) {
            
    //     }
    // }
    server::start::start()
}