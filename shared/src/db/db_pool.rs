//! db pool for one database
//! 
#![allow(unused_imports, unused_variables, dead_code)]
use futures::future::BoxFuture;
use futures::TryFutureExt;
use sqlx::mysql::MySqlRow;
use sqlx::mysql::MySqlConnectOptions;
use std::ops::Deref;
use std::{future::{Future}, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use mio::*;
use sqlx::*;
use super::*;
use std::task::Poll;
use once_cell::sync::{OnceCell};
use std::collections::BTreeMap;
use crossbeam::channel::*;
use futures_util::FutureExt;
use std::str::FromStr;
use tokio::{runtime::{Runtime, Builder}};
static COMMAND_SQC: AtomicUsize = AtomicUsize::new(1);
enum CommandState {
    ///prepare to query
    PREPARE,
    ///quering
    QUERY,
    ///finished query
    FINISH,
}
// struct Command{
//     stmt: StmtQuery,
//     state: Poll<CommandState>,
//     state_sender: Option<Sender<StmtResult>>,
//     state_handler: Receiver<StmtResult>,
// }
struct Pool{
    #[allow(unused)]
    db: String,
    name: String,
    ///current queries on this pool
    //queries: BTreeMap<usize,Command>,
    mysql_pool: Arc<sqlx::mysql::MySqlPool>,
}
unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

static POOL_HANDLERS: OnceCell<Arc<BTreeMap<String,Pool>>> = OnceCell::new();
pub struct DbPoolInfo{
    pub max_conn: u32,
    pub db_path: String,
    pub db_name: String,
}
pub struct PooledDbInfo{
    pub db_name: String,
    pub pool_id: usize,
}
static POOL_INDEX: AtomicUsize = AtomicUsize::new(1);
///db 运行时
static RUNTIME: OnceCell<Arc<Runtime>> = OnceCell::new();
///start a database pool 
/// ```
/// shared::db::start_pool(vec![DbPoolInfo{db_path: "mysql://test:test0123@127.0.0.1:3306/db_test".to_string(), db_name: "db_test".to_string(),max_conn: 20}])
/// ```
#[allow(unused_mut)]
pub fn start_pools(pools: Vec<DbPoolInfo>) -> Vec<PooledDbInfo>{
    let mut result = Vec::new();
    let mut pooled = Vec::new();
    let mut builder = Builder::new_multi_thread();
    let num = num_cpus::get();
    let runtime = builder.worker_threads(num).enable_all().build().expect("fail to build runtime");
    for db in pools.into_iter() {
        let pool_id = POOL_INDEX.fetch_add(1, Ordering::Release);
        let poll = mio::Poll::new().unwrap();    
        let waker = Waker::new(poll.registry(),Token(1)).expect("fail to create database pool waker!");
        let mut option = MySqlConnectOptions::from_str(&db.db_path).map_err(|e| e.to_string()).expect(&("fail to parse db_path ".to_string()+ &db.db_path));
        option.disable_statement_logging();
        let (tx,rx) = bounded(1);
        let max_conn = db.max_conn;
        runtime.spawn(async move {
            let mysql_pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(max_conn)
            .idle_timeout(std::time::Duration::from_secs(15*60))
            .after_connect(|conn|{
                Box::pin(async move {
                    conn.execute("
                    SET time_zone='+8:00';
                        ").await?;
                    Ok(())
                })
            })
            .connect_lazy_with(option);
            tx.send(mysql_pool).map_err(|_| ()).expect("fail to init pool");
        });
        let mysql_pool = rx.recv().map_err(|_| ()).expect("fail to create mysql pool");
        result.push(PooledDbInfo{db_name: db.db_name.clone(),pool_id});
        let pool = Pool{
            name: db.db_name,
            db: db.db_path, 
            mysql_pool: Arc::new(mysql_pool)
        };
        pooled.push(pool);
    }
    RUNTIME.set(Arc::new(runtime)).map_err(|_| ()).expect("tokio runtime 已设置");
    POOL_HANDLERS.set(Arc::new(pooled.into_iter().map(|db| (db.name.clone(),db)).collect())).map_err(|_| ()).expect("数据库已经初始化!");
    result
}
///```
/// //send mysql query
/// ```
pub fn send_query(fut: BoxFuture<'static,()>)-> anyhow::Result<()>{
    match RUNTIME.get(){
        None => crate::error::broken_pipe(),
        Some(runtime) => {
            runtime.spawn(fut);
            Ok(())
        }
    }
}
pub fn get_pool(db_name: &str) -> anyhow::Result<Arc<sqlx::mysql::MySqlPool>>{
    match POOL_HANDLERS.get(){
        None => crate::error::broken_pipe(),
        Some(handlers) => {
            match handlers.get(db_name){
                None => crate::error::broken_pipe(),
                Some(pool)=> {
                    Ok(pool.mysql_pool.clone())
                }
            }
        }
    }
}