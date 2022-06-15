//! 探索事件

use std::fmt::Debug;
use shared::map::Point2;
use shared::proto::EExploreEventType;
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GameEventState{
    Unfinished,
    Finished,
}

///事件信息
#[derive(Copy, Clone, Debug, Eq)]
pub struct EventInfo{
    pub id: u64,
    pub cfg_id: u32,
    pub map_id: u32,
    pub position: Point2,
    pub state: GameEventState,
    pub event_type: EExploreEventType,
}
impl PartialEq for EventInfo{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
#[derive(Debug)]
pub struct ExploreEvent{
    ///数据库唯一id
    pub id: u64,
    ///探索事件id
    pub explore_id: u64,
    pub map_id: u32,
    pub state: GameEventState,
    pub event_id: u32,
    pub event_type: EExploreEventType,
    ///事件是否已通知玩家
    pub notify_state: bool,    
    pub position: Point2,
    pub progress_event: bool,
}
impl PartialEq for ExploreEvent{
    fn eq(&self, other: &Self) -> bool {
        self.event_id == other.event_id && self.event_type == other.event_type
    }
}
impl ExploreEvent{
    pub fn new(
        id: u64, 
        map_id: u32,
        event_id: u32, 
        event_type: EExploreEventType, 
        state: GameEventState, 
        position: Point2, 
        progress_event: bool,
    ) -> Self{ 
        Self { 
            id: 0,
            map_id,
            explore_id: id, 
            state: state, 
            event_id, 
            event_type, 
            notify_state: state != GameEventState::Unfinished,
            position,
            progress_event,
        }
    }
}
impl Into<shared::proto::Es2CMsgEventSync> for &ExploreEvent{
    fn into(self) -> shared::proto::Es2CMsgEventSync {
        let mut locate = shared::proto::Point2::new();
        locate.x = self.position.x;
        locate.y = self.position.y;
        shared::proto::Es2CMsgEventSync{
            location: protobuf::SingularPtrField::some(locate),
            event_type: self.event_type, 
            event_id: self.event_id,
            uuid: self.id,
            ..Default::default()
        }
    }
}
impl Into<shared::proto::Es2cMsgExploreNewEvent> for &ExploreEvent{
    fn into(self) -> shared::proto::Es2cMsgExploreNewEvent {
        shared::proto::Es2cMsgExploreNewEvent{
            x: self.position.x,
            y: self.position.y, 
            uuid: self.id,
            ..Default::default()
        }
    }
}