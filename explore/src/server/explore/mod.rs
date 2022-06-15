mod explore;
mod explore_player;
mod explore_event;
mod db_handler;
mod trigger;
use std::sync::atomic::{Ordering, AtomicU64};
pub use explore::*;
pub mod player_session;
///探索id
static EXPLORE_ID:AtomicU64 =  AtomicU64::new(1);
///获取探索id
pub(crate) fn get_uuid(server_id: u32) -> u64{
    let flag = (server_id as u64) << 48u64;
    let ms = shared::get_current_ms();
    let mut explore_id = ms.abs() as u64 | flag;
    while EXPLORE_ID.load(Ordering::Acquire) >= explore_id{
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ms = shared::get_current_ms();
        explore_id = ms.abs() as u64 | flag;
    }
    EXPLORE_ID.store(explore_id, Ordering::Release);
    explore_id
}
