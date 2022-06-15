//! rust服务配置文件模块.
//! 自动生成代码,请勿修改.
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused)]
pub mod monster;
pub mod partner;
pub mod common;
pub use{
    monster::MonsterConfig,
    partner::PartnerConfig,
    common::CommonConfig,
};

/// 获取配置id
/// 
pub trait IConfig{
    fn id(&self) -> u32;
}
    