//! 服务器时间系统
use chrono::{Local, Timelike};



///获取本地时间戳
#[allow(unused)]
#[inline]
pub fn get_timestamp_now() -> i64 {
    Local::now().timestamp_millis()
}
///get timestamp ms of today from 0:0:0
pub fn get_timestamp_of_today() -> i64{
    Local::now().num_seconds_from_midnight() as i64 * 1000
}
#[inline]
pub const fn one_day_time() -> i64{
    const ONE_DAY:i64 = 24*60*60*1000;
    ONE_DAY
}
#[inline]
pub fn get_current_ms() -> i64{
    crate::get_timestamp_now()
}