//! timers

use crate::if_else;
/// interval timer
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq)]
pub struct IntervalTimer{
    interval: i64,
    current: i64,
}
impl IntervalTimer{
    pub fn new(interval: i64) -> Self{
        Self{
            interval, current: 0
        }
    }
    #[inline]
    pub fn passed(&self) -> bool{
        self.current >= self.interval
    }
    #[inline]
    pub fn reset(&mut self){
        self.current %= self.interval
    }
    #[inline]
    pub fn set_current(&mut self, cur: i64){
        self.current = cur
    }
    #[inline]
    pub fn set_interval(&mut self, inv: i64){
        self.interval = inv;
    }
    #[inline]
    pub fn update(&mut self, diff: i64){
        self.current += diff;
        if self.current < 0{
            self.current = 0
        }
    }
    #[inline]
    pub fn get_interval(&self) -> i64{
        self.interval
    }
    #[inline]
    pub fn get_current(&self) -> i64{
        self.current
    }
}
/// periodic timer
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq)]
pub struct PeriodicTimer{
    period: i64,
    expire: i64,
}
impl PeriodicTimer{
    ///create a periodic timer with next expire start time
    pub fn new(period: i64, start_time: i64) -> Self{
        Self{period,expire: start_time}
    }
    ///return true if timer expired
    pub fn update(&mut self, diff: i64) -> bool{
        self.expire -= diff;
        if self.expire > 0{
            false
        }
        else{
            self.expire += if_else!(self.period > diff,self.period,diff);
            true
        }
    }
}
#[inline]
pub fn get_current_ms() -> i64{
    crate::get_timestamp_now()
}