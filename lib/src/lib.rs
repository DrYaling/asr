#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lib_shared;
pub mod proto;
pub mod redis;
pub mod db;
pub mod logger;
pub use lib_shared::libconfig::{config};
pub mod server;
pub use server::{
    SocketHandler,
    channel::{AsyncTransportChannel},
    session::{SocketMessage, rpc_rep_state as RpcRepState, SessionTransport}, 
    handler::{SessionHandler, MsgSendHandler, AsyncSessionHandler, SyncSessionHandler, SyncSocketHandler, AsyncSocketHandler, Transporter, AsyncSocketSendHandler, TransportReceiver}, 
    context::{AsyncContext, AsyncContextImpl}
};
pub use lib_shared::{timer, time, error};
pub use lib_shared::libconfig;

static SESSION_TOKEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
#[allow(unused)]
pub(crate) fn get_session_token(worker_token: usize) -> mio::Token{
    let session =  (SESSION_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Release) & 0x0000_0000_FFFF_FFFF) | (worker_token & 0x0000_0000_FFFF_FFFF) << 32;
    mio::Token(session)
}
///道具配置 金币
pub const ITEM_TYPE_GOLD: u32 = 1;
///道具配置 钻石
pub const ITEM_TYPE_CRYSTAL: u32 = 2;
///道具配置 食物
pub const ITEM_TYPE_FOOD: u32 = 3;
///道具配置 san值
pub const ITEM_TYPE_SAN: u32 = 4;
pub type SessionId = usize;
pub enum ObjectType{
    None    = 0,
    Player  = 1,
    Character = 2,
    Undefined = 3,
    Npc = 4,
    Monster = 5,
    WorldObject = 6,
}
impl ObjectType{
    fn as_int(self) -> u32{
        self as u32
    }
}
impl From<u64> for ObjectType{
    fn from(t: u64) -> Self {
        match t{
            1 => ObjectType::Player,
            2 => ObjectType::Character,
            3 => ObjectType::Undefined,
            4 => ObjectType::Npc,
            5 => ObjectType::Monster,
            6 => ObjectType::WorldObject,
            _ => ObjectType::None,
        }
    }
}
#[derive(Debug, Default, Eq, PartialEq, Copy, Clone)]
pub struct Guid((u64,u64));#[allow(unused)]
impl Guid{
    pub fn create(ot: ObjectType, realm_id: u64, id: u64) -> Self{
        Guid((((ot.as_int() as u64) << 32) | realm_id, id))
    }
    #[inline]
    pub fn get_type(&self) -> ObjectType{
        ((self.0.0 >> 32) & 0xffff).into()
    }
    #[inline]
    pub fn get_realm(&self) -> u64{
        (self.0.0 & 0xffffffff)
    }
}
