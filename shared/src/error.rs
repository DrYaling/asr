#[macro_export]
#[allow(unused_macros)]
macro_rules! strech_log {
    ($err: expr) => {{
        log_error!("fail:{:?} at file {}, line {}",&$err,file!(),line!());
        $err.strech()
    }};
    ($err: expr, $info: expr) => {{
        log_error!("{} fail:{:?} at file {}, line {}",&$info,&$err,file!(),line!());
        $err.strech()
    }};
}
#[inline]
///channel 发送消息错误
pub fn send_err()-> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::BrokenPipe)
}
#[inline]
///解包错误
pub fn unpack_err()-> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::InvalidData)
}
#[inline]
///send_err的Result版
pub fn send_err_result<T>() -> std::io::Result<T>{
    Err::<T,std::io::Error>(send_err())
}
#[inline]
///unpack_err的Result版
pub fn unpack_err_result<T>() -> std::io::Result<T>{
    Err::<T,std::io::Error>(unpack_err())
}
#[inline]
pub fn any_err<T, E: Into<std::io::Error>>(e: E) -> anyhow::Result<T> where std::io::Error: From<E>{
    let e = Err::<T,std::io::Error>(std::io::Error::from(e));
    let ret = e?;
    Ok(ret)
}
#[inline]
pub fn broken_pipe<T>() -> anyhow::Result<T>{
    let e = Err::<T,std::io::Error>(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    let ret = e?;
    Ok(ret)
}
#[inline]
///包装error 为anyhowError
pub fn wrap<T, E: std::error::Error + Send + std::marker::Sync + 'static>(e: E) -> anyhow::Result<T>{
    let e = Err(e)?;
    e
}
#[inline]
pub fn switch<T,B: Send + std::marker::Sync + 'static>(ret: Result<T,B>)-> anyhow::Result<T> where B: std::error::Error{
    let ret = ret?;
    Ok(ret)
}
