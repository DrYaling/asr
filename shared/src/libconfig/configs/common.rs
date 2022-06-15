//! Common 配置.
//! 自动生成代码,请勿修改.
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused)]
use serde::{Deserialize};
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CommonConfig{
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub id_server: i32,
}
