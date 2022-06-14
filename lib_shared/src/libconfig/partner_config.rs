//! 角色配置

use std::{collections::BTreeMap, io::Read};
use once_cell::sync::OnceCell;
use crate::boxed::MutableBox;
use super::configs::PartnerConfig;
static CONFIGS: OnceCell<MutableBox<BTreeMap<u32,PartnerConfig>>> = OnceCell::new();
///加载角色配置
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
    let config_map: BTreeMap<u32,PartnerConfig> = serde_json::from_str(&content)?;
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
    let config_map: BTreeMap<u32,PartnerConfig> = serde_json::from_str(&content)?;
    *configs = config_map;
    Ok(())
}
///加载角色配置
pub fn load_partner_config(config_id: u32)-> Option<PartnerConfig>{
    CONFIGS.get().or_else(|| {log_error!("partner config not initialized");None})?.get(None).or_else(|| {log_error!("ai config not initialized");None})?.get(&config_id).cloned()
}
pub fn load_partner_configs<F: Fn(&(&u32,&PartnerConfig)) -> bool>(predicate: F) -> BTreeMap<u32,PartnerConfig>{
    CONFIGS.get().expect("partner config not initialized yet!").get(None).unwrap().iter().filter(predicate).map(|(id,cfg)| (*id,cfg.clone())).collect()
}
pub fn unload_all(){
    match CONFIGS.get(){
        Some(config) => {
            *config.get_mut(None).unwrap() = Default::default();
        }
        _ => (),
    }
}