use lib_shared::map::{Point2, Map};
use super::explore_event::{ExploreEvent, EventInfo};
use super::explore_player::ExplorePlayer;
pub struct ExploreTrigger{
    event_index: u32,
    event_uid: u64,
    trigger_events: Vec<ExploreEvent>,
    player_position: Point2,
    ///玩家移动速度
    player_speed: u32,
}
impl ExploreTrigger {
    pub fn new(_event_id: u32) -> Self{
        Self{
            event_uid: 1u64,
            event_index: 1,
            trigger_events: Default::default(),
            player_position: Default::default(),
            player_speed: lib::libconfig::common::get_value("MovementSpeed").unwrap_or(10),
        }
    }
    ///initialize
    pub fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    ///触发事件
    pub fn trigger(&mut self, player: &mut ExplorePlayer, map: &Map, position: Point2){
        //如果玩家位置变化,进行同步
        if position != self.player_position{
            self.player_position = position;
        }
        //TODO
    }
    ///当前事件为空
    pub fn empty(&self) -> bool{
        //TODO
        self.trigger_events.is_empty()
    }
}
