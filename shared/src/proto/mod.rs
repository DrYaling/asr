#[allow(unused)]
use serde::{Serialize, Deserialize};
mod msg_common;
pub use msg_common::*;
mod msg_c_ls;
pub use msg_c_ls::*;
mod msg_c_ps;
pub use msg_c_ps::*;
mod msg_channel;
pub use msg_channel::*;
mod msg_ps_es;
pub use msg_ps_es::*;
mod msg_c_es;
pub use msg_c_es::*;
pub use protobuf::Message as Message;
pub use crate::net_core::*;
pub mod proto_code{
    ///公共心跳包id
    pub const HEART: u16        = 1;
    ///缺省主协议号
    pub const DEFAULT_MAIN_CODE: u16 = 11;
    ///服务channel连接包
    pub const MSG_CHANNEL_CONNECT: u16 = 5032;
    ///平台服到探索服的消息id
    pub mod msg_id_es_ps{
        ///到探索服的创建探索消息id
        pub const CREATE_EXPLORE_REQ: u16 = 1022;
        ///创建探索返回
        pub const CREATE_EXPLORE_RESP: u16 = 1023;
        ///战斗胜利
        pub const FIGHT_SUCCESS_REQ: u16 = 1024;
        ///战斗胜利返回
        pub const FIGHT_SUCCESS_RESP: u16 = 1025;
        ///探索结束
        pub const EXPLORE_END_SYNC: u16 = 1026;
    }
}
