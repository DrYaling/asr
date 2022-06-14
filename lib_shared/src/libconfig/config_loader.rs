//! 通用配置加载器

use std::{collections::BTreeMap, io::Read};

use once_cell::sync::OnceCell;
use crate::boxed::MutableBox;
use super::configs::IConfig;
///配置加载器
#[derive(Default)]
pub struct ConfigLoader<T: IConfig + 'static>{
    configs: BTreeMap<u32, T>,
}
impl<T: IConfig + 'static> ConfigLoader< T>{
    ///读取配置文本
    pub fn read_config(path: &str) -> anyhow::Result<String>{
        let mut file = std::fs::File::open(path).map_err(|e|logthrow!(e,e))?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }
    ///重载配置
    pub fn reload<'a>(&mut self, content: &'a str) -> anyhow::Result<()> where T: serde::Deserialize<'a>{
        let config_map: BTreeMap<u32,T> = serde_json::from_str(content)?;
        self.configs = config_map;
        Ok(())
    }
    ///加载配置
    pub fn load_map<'a>(content: &'a str) -> anyhow::Result<Self> where T: serde::Deserialize<'a>{
        let config_map: BTreeMap<u32,T> = serde_json::from_str(content)?;
        Ok(
        ConfigLoader{
                configs: config_map,
            }
        )
    }
    ///手动写入配置
    #[inline]
    pub fn insert(&mut self, id: u32, value: T) -> Option<T>{
        self.configs.insert(id, value)
    }
    #[inline]
    ///获取配置
    pub fn get_config(&self, config_id: u32)-> Option<&T>{
        self.configs.get(&config_id)
    }
    #[inline]
    pub fn get_configs<F: Fn(&(&u32,&T)) -> bool>(&self, predicate: F) -> BTreeMap<u32,&T>{
        self.configs.iter().filter(predicate).map(|(id,cfg)| (*id,cfg)).collect()
    }
    #[inline]
    ///查找配置
    pub fn find_config<F: Fn(&(&u32,&T)) -> bool>(&self, predicate: F) -> Option<&T>{
        self.configs.iter().find(predicate).map(|(id,cfg)| cfg)
    }
    #[inline]
    ///获取配置数量
    pub fn len(&self) -> usize { self.configs.len() }
    pub fn unload(&mut self){
        self.configs = BTreeMap::new();
    }
}