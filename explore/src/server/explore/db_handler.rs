use shared::map::Point2;
use sqlx::{Row, mysql::MySqlRow};
use futures::FutureExt;
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ExploreEventInfo{
    pub id: u64,
    pub scene_id: u64,
    pub event_id: u32,
    pub event_type: i32,
    pub position: sqlx::types::Json<Point2>,
    pub progress_event: i32,
}
impl From<&super::explore_event::ExploreEvent> for ExploreEventInfo{
    fn from(evt: &super::explore_event::ExploreEvent) -> Self {
        Self{
            id: evt.id,
            scene_id: evt.map_id as u64,
            event_id: evt.event_id,
            event_type: evt.event_type as i32,
            position: sqlx::types::Json(evt.position), 
            progress_event: if_else!(evt.progress_event, 1, 0)
        }
    }
}
///探索进行中状态
pub const EXPLORE_STATE_NORMAL: i32 = 0;
///探索完成状态
pub const EXPLORE_STATE_FINISHED: i32 = 1;
///探索取消状态
pub const EXPLORE_STATE_REMOVED: i32 = 2;
#[derive(Debug)]
pub struct ExploreInfo{
    pub id: u64,
    pub player_id: u64,
    pub state: i32,
    pub explore_id: u32,
    pub max_event: i32,
    pub finished_event: i32,
    pub create_time: chrono::DateTime<chrono::Local>,
    pub token: String,
    pub finished_events: Vec<ExploreEventInfo>,
    pub variables: Vec<(i32,i32)>,
    pub position: Option<sqlx::types::Json<Point2>>,
    pub food: i32,
    ///已完成事件
    pub unique_events: Vec<(i32,u32)>,
}
impl Default for ExploreInfo{    
    fn default() -> Self {
        Self{
            create_time: chrono::Local::now(),
            id: 0,
            player_id: 0,
            state: 0,
            explore_id: 0,
            max_event: 0,
            finished_event: 0,
            token: "".to_string(),
            finished_events: Default::default(),
            variables: Default::default(),
            position: Default::default(),
            food: 0,
            unique_events: Default::default(),
        }
     }
}
impl<'a> sqlx::FromRow<'a, MySqlRow> for ExploreInfo{
    fn from_row(row: &'a MySqlRow) -> Result<Self, sqlx::Error> {
        Ok(Self{
            id: row.try_get("id").unwrap_or_default(),
            player_id: row.try_get("player_id").unwrap_or_default(),
            state: row.try_get("state").unwrap_or_default(),
            explore_id: row.try_get("explore_id").unwrap_or_default(),
            max_event: row.try_get("max_event").unwrap_or_default(),
            finished_event: row.try_get("finished_event").unwrap_or_default(),
            create_time: row.try_get("create_time").unwrap_or(chrono::Local::now()),
            token: row.try_get("token").unwrap_or_default(),
            position: row.try_get("position").unwrap_or_default(),
            food: row.try_get("food").unwrap_or_default(),
            finished_events: Default::default(),
            unique_events: Default::default(),
            variables: Default::default(),
        })
    }
}
impl ExploreInfo{
    pub fn get_events(&self) -> &Vec<ExploreEventInfo>{
        &self.finished_events
    }
}
#[derive(Default, Debug)]
pub struct DbHandler{
}
impl DbHandler{
    #[allow(unused, dead_code)]
    ///查询玩家登录信息
    pub fn save_explore_info(explore: ExploreInfo)-> anyhow::Result<()>{
        //暂时关闭探索保存功能
        shared::db::send_query(Box::new(async move {
            DbHandler::on_save_explore(&explore).await
            .map_err(|e| error!("fail {:?} to save explore info {:?}",e,explore)).ok();
        }).boxed())?;
        Ok(())
    }
    ///load player, if not exist, create player
    async fn on_save_explore(explore: &ExploreInfo) -> anyhow::Result<()>{
        let pool = shared::db::get_pool("db_explore")?;
        let _time = if_else!(explore.state == EXPLORE_STATE_FINISHED,Some(chrono::Local::now()),None);
        //暂不保存探索信息
        let mut trans = pool.begin().await.map_err(|e| logthrow!(e,e))?;
         sqlx::query("UPDATE db_explore SET finished_event=? WHERE id=?")
        .bind(explore.finished_events.len() as i32)
        .bind(explore.id)
        .execute(&mut trans).await.map_err(|e| logthrow!(e,e))?;
        for event in explore.get_events() {
            let count = sqlx::query_as::<_,(i32,)>("SELECT COUNT(*) FROM db_finished_event WHERE player_id=? AND event_type=? AND event_id=?")
            .bind(explore.player_id).bind(event.event_type).bind(event.event_id)
            .fetch_one(&mut trans).await.map_err(|e| logthrow!(e,e))?.0;
            if count > 0{
                continue;
            }
            sqlx::query("INSERT INTO db_finished_event 
            (player_id,scene_type,scene_id,event_id,event_type,`position`,progress_event) 
            VALUES(?,0,?,?,?,?,?,?,?)")
            .bind(explore.player_id)
            .bind(explore.explore_id)
            .bind(event.event_id)
            .bind(event.event_type)
            .bind(serde_json::to_string(&event.position.0)?)
            .bind(event.progress_event)
            .execute(&mut trans).await.map_err(|e| logthrow!(e,e)).ok();
        }
        let player_id = explore.player_id;
        let explore_id = explore.explore_id;
        let variable_list = explore.variables.iter().map(|t| format!("({},0,{},{},{})",player_id,explore_id,t.0,t.1)).collect::<Vec<_>>();
        if variable_list.len() > 0{
            let variables = format!("REPLACE INTO global_explore_variables (player_id,scene_type,scene_id,variable_type,value) VALUES {}",
            variable_list.join(","));
            sqlx::query(&variables)
           .execute(&mut trans).await.map_err(|e| logthrow!(e,e))?;
        }
        trans.commit().await.map_err(|e| logthrow!(e,e))?;
        Ok(())
    }
    ///加载或者创建探索
    pub async fn on_create_explore(player_id: u64, explore_id: u32, token: &str, base_point: Point2) -> anyhow::Result<ExploreInfo>{
        info!("load explore {:?} ", (player_id, explore_id, token));
        let pool = shared::db::get_pool("db_explore")?;
        let current = sqlx::query_as::<_,ExploreInfo>("SELECT * FROM db_explore WHERE player_id=? AND state=0")
        .bind(&player_id)
        .fetch_optional(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
        let mut explore = if current.is_some(){
            info!("explore loaded {:?}",current);
            let mut current = current.unwrap();
            let events = sqlx::query_as::<_,ExploreEventInfo>("SELECT * FROM db_finished_event WHERE player_id=? AND progress_event=1 AND scene_type=0 AND scene_id=?")
            .bind(player_id).bind(current.explore_id)
            .fetch_all(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
            current.finished_events = events;
            current.variables = sqlx::query_as("SELECT variable_type,`value` FROM global_explore_variables WHERE player_id=? AND scene_type=0 AND scene_id=?")
            .bind(player_id).bind(current.explore_id)
            .fetch_all(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
            current
        }
        else{
            let food = shared::libconfig::common::get_value("DefaultFood").unwrap_or(100);
            let query = sqlx::query("INSERT INTO db_explore (player_id,explore_id,token,food,`position`) VALUES(?,?,?,?,?)")
            .bind(player_id).bind(explore_id).bind(token).bind(food).bind(serde_json::to_string(&base_point)?)
            .execute(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
            info!("insert new explore {}", query.last_insert_id());
            let mut explore = ExploreInfo::default();
            explore.id = query.last_insert_id();
            explore.player_id = player_id;
            explore.explore_id = explore_id;
            explore.food = food;
            explore.position = Some(sqlx::types::Json(base_point));
            explore
        };
        explore.unique_events = sqlx::query_as("SELECT event_type,event_id FROM db_unique_event WHERE player_id=?")
        .bind(player_id)
        .fetch_all(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
        info!("on_create_explore {:?}", explore);
        Ok(explore)
    }
    ///保存已入队角色
    pub async fn save_character(player_id: u64, charactes: &Vec<u32>) -> anyhow::Result<()>{
        let pool = shared::db::get_pool("db_explore")?;
        for cid in charactes {
            let count = sqlx::query_as::<_,(i32,)>("SELECT COUNT(*) FROM db_player_character WHERE player_id=? AND role_id=?")
            .bind(player_id).bind(cid)
            .fetch_one(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?.0;
            if count > 0{
                continue;
            }
            sqlx::query("INSERT INTO db_player_character (role_id,player_id,own_type,state) VALUES(?,?,1,1)")
            .bind(cid).bind(player_id)
            .execute(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
        }
        info!("player explore {} save_character {:?}", player_id, charactes);
        Ok(())
    }
    ///quit explore ,ignore error event at present
    pub async fn on_quit_explore(explore_id: u64) -> anyhow::Result<()>{
        let pool = shared::db::get_pool("db_explore")?;
        sqlx::query("UPDATE db_explore SET state=2 WHERE id=?")
        .bind(explore_id)
        .execute(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
        Ok(())
    }
}