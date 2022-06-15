use std::{sync::Arc, thread::Thread};


struct ThreadWaker(Thread);

impl futures_util::task::ArcWake for ThreadWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.0.unpark();
    }
}
pub struct AsyncWaker;
impl std::future::Future for AsyncWaker{    
    type Output = core::task::Waker;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::task::Poll::Ready(cx.waker().clone())
    }
}
pub type Waker = core::task::Waker;