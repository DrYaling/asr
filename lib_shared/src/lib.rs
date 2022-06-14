//! library use by both client and server
//! created at 2021/12/27 by zxb
#[macro_use]
extern crate serde;
#[macro_use]
extern crate log;
#[allow(unused)]
#[macro_use]
extern crate anyhow;
#[macro_use]
pub mod macros;
pub mod map;
pub mod timer;
pub mod time;
pub mod boxed;
pub mod error;
pub mod libconfig;
pub mod ini;
pub mod attribute;
pub mod net_core;
pub mod discard;
pub mod aes;
pub use time::{get_timestamp_now, get_timestamp_of_today, one_day_time, get_current_ms};
mod weight;
pub use weight::WeightCalculater;
///条件检查
#[allow(unused)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConditionCheck<T: Eq + Ord>{
    Bigger(T),
    Equal(T),
    Smaller(T),
    BiggerOrEqual(T),
    SmallerOrEqual(T),
    NotEqual(T),
}
#[allow(unused)]
impl<T: Eq + Ord> ConditionCheck<T>{
    ///1大于等于2小于等于3大于4小于5等于6不等于
    pub fn new(condition: i32, check_value: T) -> Self{
        match condition{
            1 => Self::BiggerOrEqual(check_value),
            2 => Self::SmallerOrEqual(check_value),
            3 => Self::Bigger(check_value),
            4 => Self::Smaller(check_value),
            5 => Self::Equal(check_value),
            _ => Self::NotEqual(check_value)
        }
    }
    #[inline]
    pub fn valid(&self, b: &T)-> bool{
        match self{
            ConditionCheck::Bigger(a) => a > b,
            ConditionCheck::Equal(a) => a == b,
            ConditionCheck::Smaller(a) => a < b,
            ConditionCheck::BiggerOrEqual(a) => a >= b,
            ConditionCheck::SmallerOrEqual(a) => a <= b,
            ConditionCheck::NotEqual(a) => a != b,
        }
    }
}
