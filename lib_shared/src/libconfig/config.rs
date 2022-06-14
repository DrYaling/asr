use std::str::FromStr;
use core::ops::Deref;
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};
fn _get_config() -> Arc<BTreeMap<String, String>> {
    let mut file =
        File::open(config_path(None)).expect(&text!("{} 文件不存在", config_path(None)));
    let mut config_str = String::new();
    file.read_to_string(&mut config_str).expect("读取文件失败");
    let configs = crate::ini::parse(&config_path(None)).expect("parse config fail!");
    Arc::new(configs)
}
static CONFIG_PATH: Lazy<Mutex<BTreeMap<i32, String>>> = Lazy::new(|| {
    let mut bt = BTreeMap::new();
    bt.insert(0, String::from("config_dev.json"));
    Mutex::new(bt)
});
static PARAMETERS: Lazy<Arc<BTreeMap<String, String>>> = Lazy::new(_get_config);
pub fn config_path(path: Option<&str>) -> String {
    let mut sconfig = CONFIG_PATH.lock().unwrap();
    if let Some(p) = path {
        sconfig.insert(0, String::from(p));
    }
    sconfig.get(&0).unwrap().clone()
}
pub fn get_all() -> BTreeMap<String, String> {
    PARAMETERS.deref().deref().clone()
}
pub fn get<T: FromStr>(key: &str) -> Option<T> {
    PARAMETERS.get(key).map(|v| T::from_str(&v).ok()).unwrap_or_default()
}
pub fn get_str(key: &str) -> Option<String>{
    PARAMETERS.get(key).cloned()
}