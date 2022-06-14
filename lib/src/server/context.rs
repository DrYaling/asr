//! socket消息驱动的会话
//! 

use std::fmt::Debug;
use std::marker::PhantomData;

use crate::{SessionTransport, SocketMessage};

use super::{handler::AsyncSessionHandler, session::TransferTemplate};
use async_trait::async_trait;
pub trait AsyncContextBuilder{

}
///async context impl
#[async_trait]
pub trait AsyncContextImpl<T, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> where T: AsyncContextBuilder + Sized{
    fn new(_: T) -> Self;
    ///消息处理
    async fn deal_msg(&mut self, msg: SocketMessage<S>, handler: &mut Option<AsyncSessionHandler<S>>) -> anyhow::Result<()>;
    ///自定义消息
    /// 
    /// 包括handler处理
    async fn context_check(&mut self, handler: &mut Option<AsyncSessionHandler<S>>) -> anyhow::Result<()>;
    ///init context, reset handler, room mode
    #[inline]
    #[allow(unused)]
    fn update_handler(&mut self) -> Option<AsyncSessionHandler<S>>{ None }
    fn on_close(&mut self);
}
///异步会话
pub struct AsyncContext<T, E, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> where T: AsyncContextImpl<E, S>, E: AsyncContextBuilder{
    inner: T,
    handler: Option<AsyncSessionHandler<S>>,
    phantom: PhantomData<E>,
}
unsafe impl<T, E, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> Send for AsyncContext<T, E, S> where T: AsyncContextImpl<E, S>, E: AsyncContextBuilder{}
unsafe impl<T, E, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> Sync for AsyncContext<T, E, S> where T: AsyncContextImpl<E, S>, E: AsyncContextBuilder{}
impl<T, E, S: Sized + Debug + Send + Sync + 'static + TransferTemplate> AsyncContext<T, E, S> where T: AsyncContextImpl<E, S>, E: AsyncContextBuilder{
    pub fn new(handler: Option<AsyncSessionHandler<S>>, builder: E) -> Self{
        Self{
            inner: T::new(builder),
            handler,
            phantom: PhantomData
        }
    }
    pub fn from(handler: Option<AsyncSessionHandler<S>>, t: T) -> Self{
        Self{
            inner: t,
            handler,
            phantom: PhantomData
        }
    }
    ///运行会话
    pub fn start(mut self) -> anyhow::Result<()> where T: 'static + Send, E: 'static + Send{
        if let Some(handler) = self.inner.update_handler(){
            self.handler = handler.into()
        }
        super::worker::async_worker::start_context(self);
        Ok(())
    }
    pub fn inner(&self) -> &T{
        &self.inner
    }
    pub fn inner_mut(&mut self)-> &mut T{
        &mut self.inner
    }
    ///run AsyncContext
    pub(crate) async fn run(self) -> anyhow::Result<()>{
        let Self{
            mut inner,
            mut handler,
            ..
        } = self;
        if let Err(e) = Self::update(&mut inner, &mut handler).await{
            inner.on_close();
            info!("session {} disconnected for reason {:?}",handler.as_ref().map(|t| t.id()).unwrap_or_default(), e);
        }
        handler.map(|h| h.send(SessionTransport::disconnect()).ok());
        Ok(())
    }
    async fn update(inner: &mut T, session_handler: &mut Option<AsyncSessionHandler<S>>) -> anyhow::Result<()>{
        while !super::worker::stopped(){
            if let Some(handler) = session_handler{
                let mut none = None;
                tokio::select! {
                    ret = handler.recv_async() => {
                        let msg = ret?;
                        inner.deal_msg(msg, session_handler).await?;
                    },
                    //如果上一个连接还未释放,新的连接来了会走到这里
                    check = inner.context_check(&mut none) => {
                        if let Some(new_handler) = none{
                            //收到新的handler,旧的handler直接销毁了
                            let _ = std::mem::replace(handler, new_handler);
                        }
                        check?;
                    },
                }
            }
            else{
                inner.context_check(session_handler).await?;
            }
        }
        Ok(())
    }
    ///set handler, this action should be called before run
    pub fn set_handler(&mut self, handler: AsyncSessionHandler<S>){
        self.handler = handler.into();
    }
}
#[cfg(test)]
mod context_test{
    use crate::{AsyncContextImpl, SocketMessage, server::session::TransferTemplate};
    struct Builder;
    impl super::AsyncContextBuilder for Builder {}
    impl TransferTemplate for Builder{}
    struct AsyncContextTest(i32);
    #[async_trait]
    impl AsyncContextImpl<Builder, ()> for AsyncContextTest{
        fn new(_: Builder) -> Self {
            Self(0)
        }
        #[inline]
        async fn deal_msg(&mut self, _: crate::SocketMessage<()>, _: &mut Option<crate::AsyncSessionHandler<()>>) -> anyhow::Result<()> {
            self.0 = 10000000;
            Ok(())
        }

        async fn context_check(&mut self, _: &mut Option<crate::AsyncSessionHandler<()>>) -> anyhow::Result<()> {
            todo!()
        }

        fn on_close(&mut self) {
            self.0 = 10000000;
        }
    }
    impl AsyncContextTest{
        async fn deal(&mut self){           
            self.0 = 10000000;
        }
        #[inline]
        fn close(&mut self){            
            self.0 = 10000000;
        }
    }
    #[test]
    fn run_test(){
        let loop_count = 1000;
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("fail to start runtime");
        rt.block_on(async{
            let mut ctx = AsyncContextTest(0);
            let mut x = None;
            let ins = std::time::Instant::now();
            for _ in 0..loop_count {
                ctx.deal_msg(SocketMessage::OnDisconnect, &mut x).await.ok();
            }
            println!("async_trait deal_msg cost {} mills",ins.elapsed().as_millis());
        });
        {
            let mut ctx = AsyncContextTest(0);
            let ins = std::time::Instant::now();
            for _ in 0..loop_count {
                ctx.on_close();
            }
            println!("on_close cost {} mills",ins.elapsed().as_millis());
        }
        rt.block_on(async{
            let mut ctx = AsyncContextTest(0);
            let ins = std::time::Instant::now();
            for _ in 0..loop_count {
                ctx.deal().await;
            }
            println!("async deal cost {} mills",ins.elapsed().as_millis());
        });
        {
            let mut ctx = AsyncContextTest(0);
            let ins = std::time::Instant::now();
            for _ in 0..loop_count {
                ctx.close();
            }
            println!("close cost {} mills",ins.elapsed().as_millis());
        }
        rt.block_on(async {
            let (tx,mut rx) = tokio::sync::mpsc::unbounded_channel();
            let (tx1,mut rx1) = tokio::sync::mpsc::unbounded_channel();
            rt.spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                for _ in 0..1000 {
                    tx.send(1).ok();
                }
            });
            rt.spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                for _ in 0..1000 {
                    tx1.send(2).ok();
                }
            });
            let mut count = 0;
            let mut quit = 0;
            loop{
                tokio::select! {
                    rr = rx.recv() => {
                        if rr.is_some(){
                            quit += 1;
                            count += rr.unwrap();
                        }
                        if quit >= 2000{
                            break;
                        }
                    },
                    r11 = rx1.recv() => {
                        if r11.is_some(){
                            quit += 1;
                            count += r11.unwrap();
                        }
                        if quit >= 2000{
                            break;
                        }
                    }
                }
            }
            println!("sum {}",count);
        });
    }
}