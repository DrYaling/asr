///丢弃结果
pub trait Discard where Self: Sized{
    fn discard(self){}
}
impl<T> Discard for Option<T> {}
impl<R, T> Discard for Result<R, T> {}