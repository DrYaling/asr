use once_cell::sync::OnceCell;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::runtime::Runtime;

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::signal;

use super::{handler::SyncSessionHandler, session::TransferTemplate};

pub(crate) static RUNTIME: OnceCell<Arc<Runtime>> = OnceCell::new();
///服务关闭消息
static SHUTDOWN_HANDLER: OnceCell<tokio::sync::broadcast::Sender<ServiceCommand>> = OnceCell::new();
///获取关闭通知
pub fn get_shutdown_handler() -> tokio::sync::broadcast::Receiver<ServiceCommand> {
    SHUTDOWN_HANDLER.get().unwrap().subscribe()
}
static RUNNING: AtomicBool = AtomicBool::new(false);
#[derive(Debug, Clone)]
pub enum ServiceCommand {
    Shutdown,
}
pub fn block_on<F: futures::Future>(future: F) -> F::Output {
    RUNTIME.get().unwrap().block_on(future)
}
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: futures::Future + Send + 'static,
    F::Output: Send + 'static,
{
    RUNTIME.get().unwrap().spawn(future)
}
///初始化worker 工作线程
pub fn init(mut worker_count: usize) -> anyhow::Result<()>{
    if RUNTIME.get().is_some(){
        return Err(anyhow::anyhow!("runtime started!"));
    }
    if worker_count == 0{
        worker_count = num_cpus::get();
    }
    let runtime = Builder::new_multi_thread()
        .worker_threads(worker_count)
        .enable_all()
        .build()
        .expect("fail to build runtime");
    RUNTIME
        .set(Arc::new(runtime))
        .map_err(|_| ())
        .expect("tokio runtime 已设置");
    log_info!("worker started {} thread worker", worker_count);
    Ok(())
}
///```
/// //run service with address and worker count
/// //if worker count  is set to zero, then use cpu core number as worker counts
/// ```
pub fn run<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(address: &str, sync_mode: bool) {
    RUNNING.store(true, Ordering::Release);
    let addr = address.to_string();
    let (broadcast_tx, _) = tokio::sync::broadcast::channel(300000);
    SHUTDOWN_HANDLER.set(broadcast_tx).expect("服务已启动");
    log_info!("start server address {}",address);
    spawn(async move {
        tokio::select! {
            res = run_server::<S>(addr, sync_mode) => {
                if let Err(err) = res {
                    error!("failed to accept {:?}",err);
                }
            }
            _ = signal::ctrl_c() => {
                stop();
                info!("server shutdown");
            }
        };
        SHUTDOWN_HANDLER
            .get()
            .unwrap()
            .send(ServiceCommand::Shutdown)
            .ok();
    });
}
async fn run_server<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(addr: String, sync_mode: bool) -> std::io::Result<()>{
    let listener = TcpListener::bind(&addr).await?;
    let mut session_id: usize = 1;
    while RUNNING.load(Ordering::Relaxed) {
        let (socket, addr) = listener.accept().await?;
        trace!("accept new sync {} session {}", sync_mode, session_id);
        if sync_mode {
            let (tx, rx) = crossbeam::channel::unbounded();
            let session = super::session::Session::<_, _, S>::new(
                session_id,
                socket,
                addr,
                get_shutdown_handler(),
                rx,
                tx,
            );
            spawn(async move {
                let mut session = session;
                let handler = session.get_handler().await;
                on_session_accepted(handler);
                trace!("run new sync session {}", session_id);
                session.run(true).await
            });
        } else {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let session = super::session::Session::<_, _, S>::new(
                session_id,
                socket,
                addr,
                get_shutdown_handler(),
                rx,
                tx,
            );
            spawn(async move {
                let mut session = session;
                let handler = session.get_handler().await;
                async_worker::on_session_accepted(handler);
                trace!("run new async session {}", session_id);
                session.run(true).await
            });
        }
        session_id += 1;
    }
    std::io::Result::Ok(())
}
pub(crate) fn on_session_accepted<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(session: SyncSessionHandler<S>){
    ADD_SYNC_SESSION_HANDLER.get().expect("session handler for this worker not set").cb.call_box(Box::new(session));
}
pub trait AddSessionHandler<T> {
    fn call_box(&self, _: T);
}
impl<T, F: Fn(T)> AddSessionHandler<T> for F {
    fn call_box(&self, sender: T) {
        (*self)(sender)
    }
}
pub type SyncSessionAddCallback =
    Box<dyn AddSessionHandler<Box<dyn (::std::any::Any) + Send + Sync>>>;
pub(crate) struct SyncCallback {
    pub(crate) cb: SyncSessionAddCallback,
}
unsafe impl Send for SyncCallback {}
unsafe impl Sync for SyncCallback {}
static ADD_SYNC_SESSION_HANDLER: OnceCell<SyncCallback> = OnceCell::new();
///new session handler for this worker(sync mode)
pub fn set_sync_session_handler(callback: SyncSessionAddCallback) {
    ADD_SYNC_SESSION_HANDLER
        .set(SyncCallback { cb: callback })
        .map_err(|_| error!("session handler already set"))
        .ok();
}
pub fn stop() {
    RUNNING.store(false, Ordering::Release);
}
///服务已关闭
pub fn stopped() -> bool {
    !RUNNING.load(Ordering::Relaxed)
}
///异步worker
pub mod async_worker {
    use crate::server::{
        context::{AsyncContext, AsyncContextBuilder, AsyncContextImpl},
        handler::AsyncSessionHandler,
    };

    use super::*;
    pub type AsyncSessionAddCallback =
        Box<dyn AddSessionHandler<Box<dyn (::std::any::Any) + Send + Sync>>>;
    pub(crate) struct AsyncCallback {
        pub(crate) cb: AsyncSessionAddCallback,
    }
    unsafe impl Send for AsyncCallback {}
    unsafe impl Sync for AsyncCallback {}
    static ADD_ASYNC_SESSION_HANDLER: OnceCell<AsyncCallback> = OnceCell::new();
    ///new session handler for this worker(sync mode)
    pub fn set_async_session_handler(callback: AsyncSessionAddCallback) {
        ADD_ASYNC_SESSION_HANDLER
            .set(AsyncCallback { cb: callback })
            .map_err(|_| error!("session handler already set"))
            .ok();
    }
    pub(crate) fn on_session_accepted<S: Sized + Debug + Send + Sync + 'static + Clone + TransferTemplate>(session: AsyncSessionHandler<S>){
        ADD_ASYNC_SESSION_HANDLER.get().expect("session handler for this worker not set").cb.call_box(Box::new(session));
    }
    ///运行异步会话
    pub(crate) fn start_context<T: 'static + Send, E: 'static + Send, S: Sized + Debug + Send + Sync + 'static + TransferTemplate>(context: AsyncContext<T, E, S>) where T: AsyncContextImpl<E, S>, E: AsyncContextBuilder{
        RUNTIME.get().unwrap().spawn(async move{
            context.run().await.map_err(|e| info!("context exit {:?}",e)).ok();
        });
    }
}
