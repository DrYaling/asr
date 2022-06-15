// use futures::future::ok;
use shared::{logger, AsyncSessionHandler};

use actix_web::{post, App, HttpResponse, HttpServer, Responder};

use shared::server::worker;

///start server
pub fn start() -> anyhow::Result<()> {
    use std::env;
    shared::libconfig::config::config_path(Some("configs/explore_server.ini"));
    let log_dir = env::current_dir()
        .map_err(|e| -> std::io::Error {
            error!("failed to get current directory: {:?}", e);
            std::io::ErrorKind::BrokenPipe.into()
        })?
        .display()
        .to_string();
    logger::init(
        &log_dir,
        "ExploreServer".to_string(),
        shared::libconfig::config::get::<i32>("log_trace") == Some(1)).map_err(|e| {error!("failed to initialize logger: {}",e);2usize}).map_err(|_| -> std::io::Error {std::io::ErrorKind::BrokenPipe.into()})?;
    
    
    shared::server::worker::async_worker::set_async_session_handler(Box::new(|session: Box<dyn (::std::any::Any) + Send + Sync + 'static>|{
        if let Ok(result) = session.downcast::<AsyncSessionHandler<crate::server::explore::ExploreSharedChannel>>(){
            super::entry::on_new_session(*result)
        }
    }));

    load_configs()?;
    worker::init(0)?;
    shared::db::start_pools(vec![shared::db::DbPoolInfo{db_path: shared::libconfig::config::get_str("player_db").expect("fail to load db_path"), db_name: "bg_db_server".to_string(),max_conn: 20}]);
    let web_port: String = shared::libconfig::config::get("bind_web_port").expect("fail to load web port from config");
    worker::spawn(
        HttpServer::new(|| {
            App::new().service(reload)
        })
            .bind(web_port)
            .map_err(|e| logthrow!(e, e))
            .expect("fail to start http server")
            .workers(1)
            .run(),
    );
    let ip: String = shared::libconfig::config::get_str("bind_ip").expect("fail to load ip from config");
    let port: i32 = shared::libconfig::config::get("bind_port").expect("fail to load port from config");
    worker::run::<crate::server::explore::ExploreSharedChannel>(&(ip+":"+&port.to_string()),false);  
    super::channel::channel_service::start_up()?;
    worker::block_on(worker::get_shutdown_handler().recv())?;
    Ok(())
}
pub fn load_configs() -> anyhow::Result<()> {
    let dir: String = shared::libconfig::config::get("config_dir").expect("fail to load config dir");
    shared::libconfig::partner_config::load_config(&format!("{}/Partner.json", dir)).map_err(|e| logout!(e))?;
    shared::libconfig::common::load_config(&format!("{}/Common.json", dir)).map_err(|e| logout!(e))?;
    Ok(())
}
#[allow(unused)]
///stop server
pub fn stop() -> Result<(), std::io::Error> {
    shared::server::worker::stop();
    Ok(())
}
#[allow(unused)]
#[inline]
pub fn stopped() -> bool {
    //socket 服务关闭
    shared::server::worker::stopped()
}
#[post("/reload")]
async fn reload() -> impl Responder {
    use serde_json;
    if let Ok(_) = load_configs() {
        HttpResponse::Ok().body(serde_json::to_string("reload explore config successed").unwrap())
    } else {
        HttpResponse::BadRequest().body(serde_json::to_string("reload explore config failed").unwrap())
    }
}
