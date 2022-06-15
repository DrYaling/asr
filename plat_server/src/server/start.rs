use std::collections::VecDeque;
use std::sync::{Mutex};
use std::sync::atomic::*;
use super::world::{WorldBuilder};
use shared::get_current_ms;
use once_cell::sync::{Lazy};
use shared::{logger, SyncSessionHandler};
static WORLD_LOOP_COUNTER: AtomicU64 = AtomicU64::new(0);
const WORLD_SLEEP_CONST: u64 = 50;
static LOOP_TIMER: std::sync::atomic::AtomicU32 = AtomicU32::new(0);
static NEW_SESSION: AtomicBool = AtomicBool::new(false);
static SESSION_QUEUE: Lazy<Mutex<VecDeque<SyncSessionHandler<()>>>> = Lazy::new(|| Mutex::new(Default::default()));

fn world_loop() -> anyhow::Result<()>{
    let mut world = WorldBuilder::new().build()?;
    //channel 服务
    super::channel::channel_service::start_up()?;
    super::channel::explore_manager::start_up(world.world_cmd_handler())?;
    let mut last_time = get_current_ms();
    let mut _current_time = last_time;
    world.start().expect("fail to start plat server!");
    while !stopped() {
        //update world loop counter
        WORLD_LOOP_COUNTER.fetch_add(1, Ordering::Release);
        _current_time = get_current_ms();
        let diff = (_current_time - last_time).max(0);
        last_time = _current_time;
        super::channel::channel_service::update(diff);
        super::channel::explore_manager::update(diff);
        //main update entry
        world.update(diff as u32);
        if NEW_SESSION.load(Ordering::Acquire){
            let mut sessions = SESSION_QUEUE.lock().unwrap();
            NEW_SESSION.store(false, Ordering::Release);
            while sessions.len() > 0{
                let index =  sessions.len() - 1;
                let session = sessions.remove(index).unwrap();
                world.add_session(session);
            }
        }
        if LOOP_TIMER.fetch_add(diff as u32, Ordering::Release) > 2000{
            LOOP_TIMER.store(0, Ordering::Release);        
        }
        let update_cost = (get_current_ms() - _current_time).max(0) as u64;
        if update_cost < WORLD_SLEEP_CONST{
            std::thread::sleep(std::time::Duration::from_millis(WORLD_SLEEP_CONST-update_cost));
        }
    }
    Ok(())
}
///start world
pub fn start() -> anyhow::Result<()>{
    if stopped() == false{
        let err = Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        err?;
    }
    use std::env;
    let log_dir = env::current_dir().map_err(|e| -> std::io::Error {error!("failed to get current directory: {:?}",e);std::io::ErrorKind::BrokenPipe.into()})?.display().to_string();
    logger::init(&log_dir,"PlatServer".to_string(), false).map_err(|e| {error!("failed to initialize logger: {}",e);2usize}).map_err(|_| -> std::io::Error {std::io::ErrorKind::BrokenPipe.into()})?;
    
    shared::libconfig::config::config_path(Some("config/plat_server.ini"));
    
    shared::server::worker::set_sync_session_handler(Box::new(|session: Box<(dyn std::any::Any + std::marker::Send + std::marker::Sync + 'static)>|{
        match session.downcast::<SyncSessionHandler<()>>() {
            Ok(result) => {
                SESSION_QUEUE.lock().unwrap().push_back(*result);
                NEW_SESSION.store(true, Ordering::Release);
            }
            _ => (),
        }
    }));
    shared::libconfig::common::load_config("configs/json/Common.json")?;
    let _: String = shared::libconfig::config::get_str("explore_server_ip").expect("fail to load explore_server_ip from config");
    let _: i32 = shared::libconfig::config::get("explore_server_port").expect("fail to load explore_server_port from config");
    
    let _: String = shared::libconfig::config::get_str("battle_server_ip").expect("fail to load battle_server_ip from config");
    let _: i32 = shared::libconfig::config::get("battle_server_port").expect("fail to load battle_server_port from config");
    
    let ip: String = shared::libconfig::config::get_str("bind_ip").expect("fail to load ip from config");
    let port: i32 = shared::libconfig::config::get("bind_port").expect("fail to load port from config");
    shared::server::worker::init(0)?;
    shared::server::worker::run::<()>(&(ip+":"+&port.to_string()),true);
    //shared::server::worker::run::<()>(&(ip+":"+&port.to_string()),true);   
    shared::db::start_pools(vec![shared::db::DbPoolInfo{db_path: shared::libconfig::config::get_str("player_db").expect("fail to load db_path"), db_name: "bg_db_server".to_string(),max_conn: 20}]);
    world_loop()?;
    Ok(())
}
#[allow(unused)]
///stop world 
pub fn stop() -> Result<(),std::io::Error>{
    shared::server::worker::stop();
    Ok(())
}
#[inline]
pub fn stopped() -> bool{
    //socket 服务关闭
    shared::server::worker::stopped()
}