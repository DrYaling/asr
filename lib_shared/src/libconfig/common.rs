//! 全局常量表
use std::str::FromStr;
use std::{collections::BTreeMap, io::Read};

use once_cell::sync::OnceCell;
use crate::boxed::MutableBox;
use super::configs::CommonConfig;
static CONFIGS: OnceCell<MutableBox<BTreeMap<String,CommonConfig>>> = OnceCell::new();

///加色配置
pub fn load_config(path: &str) -> anyhow::Result<()>{
    let mut file = std::fs::File::open(path).map_err(|e|logthrow!(e,e))?;
    let mut content = String::new();
    let bytes = file.read_to_string(&mut content)?;
    let mut configs = match CONFIGS.get(){
        Some(cfg) => cfg,
        None => {
            CONFIGS.set(MutableBox::new(BTreeMap::new())).ok();
            CONFIGS.get().unwrap()
        }
    };
    let mut configs = configs.get_mut(None).unwrap();
    let config_map: BTreeMap<String,CommonConfig> = serde_json::from_str(&content)?;
    *configs = config_map;
    Ok(())
}

///加载内容配置
pub fn load_config_with_content(content: &str) -> anyhow::Result<()>{
    let mut configs = match CONFIGS.get(){
        Some(cfg) => cfg,
        None => {
            CONFIGS.set(MutableBox::new(BTreeMap::new())).ok();
            CONFIGS.get().unwrap()
        }
    };
    let mut configs = configs.get_mut(None).unwrap();
    let config_map: BTreeMap<String,CommonConfig> = serde_json::from_str(content)?;
    *configs = config_map;
    Ok(())
}
///获取配置数据,
/// 
///# 注意, string类型不能通过此接口获取
pub fn get_value<T: FromStr>(key: &str) -> Option<T>{
    match CONFIGS.get().or_else(|| {log_error!("common config not initialized");None})?.get(None).unwrap().get(key){
        None => None,
        Some(v) => {
            T::from_str(&v.value).map_err(|e| log_error!("解析common字段 {} 错误 value {}",key, v.value)).ok()
        }
    }
}
///获取配置属性
pub fn get_str(key: &str) -> Option<String>{
    CONFIGS.get().or_else(|| {log_error!("common config not initialized");None})?.get(None).unwrap().get(key).map(|cfg| cfg.value.clone())
}
pub fn unload_all(){
    match CONFIGS.get(){
        Some(config) => {
            *config.get_mut(None).unwrap() = Default::default();
        }
        _ => (),
    }
}