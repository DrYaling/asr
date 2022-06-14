//!战斗服通讯通道
#![allow(unused)]
use lib::{AsyncSessionHandler, Transporter, AsyncSocketHandler, server::{channel::{self, ServiceChannel, ChannelState}}, timer::IntervalTimer};
pub struct BattleChannel{
    handler: AsyncSessionHandler<()>,
    heart_timer: IntervalTimer,
    state: ChannelState
}