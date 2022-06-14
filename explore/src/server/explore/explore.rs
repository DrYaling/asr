//! 探索

use super::db_handler::ExploreInfo;
use super::explore_player::{explore_player_dirty_flag, ExplorePlayer};
use super::{db_handler::DbHandler, player_session::PlayerSessionInfo};
use crate::server::explore::explore_player::CharacterState;
use lib::{
    proto::PackBuffer,
    server::context::AsyncContextBuilder,
    timer::IntervalTimer,
    AsyncContextImpl, AsyncSessionHandler, SessionTransport, SocketMessage,
};
use lib_shared::map::Map;
use rand::Rng;
use super::trigger::{ExploreTrigger};
type ExploreSessionTransport = SessionTransport<()>;
///shared channel for explore room
#[derive(Debug, Clone)]
pub struct ExploreSharedChannel {
    proxy: Option<tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>>,
}
impl lib::server::session::TransferTemplate for ExploreSharedChannel {
    fn get_proxy(&mut self) -> Option<tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>> {
        self.proxy.take()
    }
}
#[derive(Debug, Default)]
pub struct ExploreSimpleInfo {
    pub token: String,
    pub id: u64,
    pub player_id: u64,
    pub server_id: usize,
}
///保存探索间隔
const SAVE_EXPLORE_INTERVAL: u64 = 2 * 60 * 1000;
///探索状态
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExploreState {
    ///等待玩家登入
    Loading(u32),
    ///探索中
    Exploring,
    ///已完成
    Finished,
    ///玩家掉线,等待重连
    Disconnected(u32),
    CreateFail,
    UnexpectedError,
    Closed,
    ///重连,重连时间
    Reconnecting(u32),
    ///探索失败
    Failed,
}
impl ExploreState {
    #[inline]
    pub fn disconnected(&self) -> bool {
        match self {
            ExploreState::Disconnected(_) => true,
            _ => false,
        }
    }
    #[inline]
    pub fn reconnecting(&self) -> bool {
        match self {
            ExploreState::Reconnecting(_) => true,
            _ => false,
        }
    }
}
impl From<i32> for ExploreState {
    fn from(v: i32) -> Self {
        match v {
            0 => ExploreState::Exploring,
            1 => ExploreState::Finished,
            2 => ExploreState::Failed,
            _ => ExploreState::Closed,
        }
    }
}
pub struct ExploreBuilder;
impl AsyncContextBuilder for ExploreBuilder {}
pub struct Explore {
    explore_id: u64,
    player_id: u64,
    player_session: usize,
    explore_cfg_id: u32,
    plat_server: usize,
    token: String,
    #[allow(unused)]
    origin_characters: Vec<u32>,
    state: ExploreState,
    player_info: ExplorePlayer,
    heart_timer: IntervalTimer,
    save_timer: IntervalTimer,
    ///当前地图
    map: Map,
    event_trigger: ExploreTrigger,
    event_handler: Option<tokio::sync::mpsc::UnboundedReceiver<SocketMessage<PlayerSessionInfo>>>,
    ///移动启程消耗食物
    move_cost: i32,
    ///移动单位距离消耗食物
    move_unit_cost: i32,
    saved: bool,
    ///保存探索信息的时间节点
    save_time: u64,
    ///已保存的完成列表
    finish_list: Vec<u64>,
    ///移动启程消耗血上限
    move_cost_hp: i32,
    ///移动单位距离消耗血上限
    move_unit_cost_hp: i32,
    sender: tokio::sync::mpsc::UnboundedSender<SocketMessage<()>>,
    receiver: Option<tokio::sync::mpsc::UnboundedReceiver<SocketMessage<()>>>,
    tsender: tokio::sync::mpsc::UnboundedSender<SessionTransport<()>>,
    #[allow(unused)]
    treceiver: tokio::sync::mpsc::UnboundedReceiver<SessionTransport<()>>,
}
#[allow(unused)]
impl Explore {
    pub fn create(
        player_id: u64,
        config_id: u32,
        server_id: usize,
        characters: Vec<u32>,
        gm_authority: u32,
        task_id: u32,
    ) -> anyhow::Result<Self> {
        let access_token = format!(
            "{:0x}{:0x}",
            player_id + config_id as u64,
            rand::thread_rng().gen::<u64>()
        );
        let map = lib_shared::map::MapBuilder::new(1, 10, 10, false).with_barriers(vec![1, 2, 3, 4, 5]).build();
        let mut pos = lib_shared::map::Point2::new(47,-7);
        map.bind_point(&mut pos);
        let explore_id = super::get_uuid(lib_shared::libconfig::config::get("server_id").unwrap_or_default());
        log_info!("create explore {} for player {} with map {}, birth location {:?}", explore_id, player_id, map.map_id(), pos);
        let (tx ,rx) = tokio::sync::mpsc::unbounded_channel();
        let (tsender ,treceiver) = tokio::sync::mpsc::unbounded_channel();
        Ok(Self{
            explore_id, player_id, state: ExploreState::Loading(0), player_session: 0,
            explore_cfg_id: config_id,
            plat_server: server_id,
            player_info: ExplorePlayer::new(
                player_id,
                config_id,
                pos,
                lib::libconfig::common::get_value("DefaultFood").unwrap_or(100),
                &characters,
                gm_authority,
            ),
            origin_characters: characters,
            token: access_token,
            heart_timer: IntervalTimer::new(10 * 1000),
            save_timer: IntervalTimer::new(60 * 1000),
            map,
            event_trigger: ExploreTrigger::new(config_id),
            event_handler: None,
            move_cost: lib::libconfig::common::get_value("MoveCost").unwrap_or(5),
            move_unit_cost: lib::libconfig::common::get_value("MoveUnitCost").unwrap_or(1),
            move_cost_hp: lib::libconfig::common::get_value("JourneyHealthLimit").unwrap_or(5),
            move_unit_cost_hp: lib::libconfig::common::get_value("MovementHealthlimit")
                .unwrap_or(1),
            saved: false,
            save_time: 0,
            finish_list: Default::default(),
            sender: tx,
            receiver: Some(rx),
            tsender,
            treceiver,
        })
    }
    #[inline]
    fn log_info(&self) -> (u64, u64, usize) {
        (self.player_id, self.explore_id, self.player_session)
    }
    #[inline]
    pub fn get_explore_uuid(&self) -> u64 {
        self.explore_id
    }
    #[inline]
    pub fn access_token(&self) -> &str {
        self.token.as_str()
    }
    #[inline]
    pub fn get_plat_server(&self) -> usize {
        self.plat_server
    }
    #[inline]
    pub fn active(&self) -> bool {
        self.state == ExploreState::Exploring || self.state.reconnecting()
    }
    #[inline]
    pub fn state(&self) -> ExploreState {
        self.state
    }
    ///玩家连接探索
    async fn append(
        &mut self,
        player_info: PlayerSessionInfo,
    ) -> anyhow::Result<AsyncSessionHandler<ExploreSharedChannel>> {
        let mut resp = lib::proto::Es2CMsgStartExploreResp::new();
        let PlayerSessionInfo {
            session_handler,
            player_id,
            token,
            rpc,
            ..
        } = player_info;
        info!(
            "player {} connected to explore, msg handler is {:p}",
            player_id, &session_handler
        );
        if token != self.token || player_info.player_id != self.player_id {
            self.state = ExploreState::CreateFail;
            error!(
                "fail to append explore ,content not fit {:?} != {:?}",
                (player_info.player_id, token),
                (&self.token, self.player_id)
            );
            resp.set_result(lib::proto::StartExploreResult::NO_EXPLORE_FOUND);
            let ret = session_handler.send(SessionTransport::new(
                lib::proto::proto_code::DEFAULT_MAIN_CODE,
                crate::msg_id::CREATE_EXPLORE_REQ_RESULT,
                rpc,
                Box::new(resp),
            ));
            session_handler.send(SessionTransport::disconnect()).ok();
            return lib::error::broken_pipe();
        } else {
            self.player_session = session_handler.id();
            self.map.bind_point(&mut self.player_info.position_mut());
            //TODO 后面再改,出生点需要保存数据库
            let pos = self.player_info.position();
            self.player_info.origin_pos = pos;
            self.state = ExploreState::Exploring;
            resp.set_result(lib::proto::StartExploreResult::START_SUCCESS);
            resp.set_seed(rand::thread_rng().gen::<i32>());
            let mut locate = lib::proto::Point2::new();
            locate.x = pos.x;
            locate.y = pos.y;
            resp.set_locate(locate);
            self.event_trigger.trigger(&mut self.player_info, &mut self.map, pos);
            let mut sync = lib::proto::Es2CMsgExploreSync::new();
            //如果探索没有结束,这里会没有角色列表
            self.player_info
                .set_dirty(explore_player_dirty_flag::CHARACTER);
            self.pack_player_info(&mut sync).await.ok();         
            if !sync.has_event_detail(){
                sync.set_event_detail(Default::default());
            }
            self.pack_sync_msg(&mut sync.mut_event_detail())
                .await
                .map_err(|e| logthrow!(e, e))
                .ok();
            resp.set_explore_info(sync);
            self.player_info.step_count = 100;
            //移除掉地图外的点id并且发送玩家视野
            resp.set_explored_map(
                self.player_info
                    .visiable_points
                    .iter()
                    .filter(|id| {
                        self.map
                            .iter()
                            .find(|point| point.id() == **id as u16)
                            .is_some()
                    })
                    .map(|f| *f as i32)
                    .collect(),
            );
            self.player_info.visiable_points.clear();
            info!("EXPLORE {:?} connect resp \r\n{:?}", self.log_info(), resp);
            session_handler.send(SessionTransport::new(
                lib::proto::proto_code::DEFAULT_MAIN_CODE,
                crate::msg_id::CREATE_EXPLORE_REQ_RESULT,
                rpc,
                Box::new(resp),
            ))?;
        }
        info!("explore {:?} connected", self.log_info());
        Ok(session_handler)
    }
    ///消息处理
    async fn handle_msg(
        &mut self,
        packet: PackBuffer,
        handler: &mut AsyncSessionHandler<()>,
    ) -> anyhow::Result<()> {
        let header = packet.header();
        let (code, rpc) = (header.sub_code() as u16, header.squence());
        let msg = match code {
            crate::msg_id::EXPLORE_MOVE_REQ => self.handle_move(packet).await?,
            lib::proto::proto_code::HEART => {
                self.heart_timer.reset();
                return Ok(());
            }
            opcode => {
                warn!(
                    "explore {:?} recv unexpected msg {}",
                    self.log_info(),
                    opcode
                );
                return Ok(());
            }
        };
        self.player_info.send_msg(msg)?;
        //探索结束
        if let Some(msg) = self.handle_explore_result() {
            info!("explore finish, resp {:?}", msg);
            // self.player_info.send_msg(msg)?;
            crate::server::channel::channel_service::send_msg(msg, self.get_plat_server())?;
            self.close()?;
            //保持连接,期间客户端可以重复查询结果
            //延迟15秒后终止探索
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            self.disconnect()?;
            return lib::error::any_err(std::io::ErrorKind::ConnectionAborted);
        }
        Ok(())
    }
    
    fn save_explore(&mut self) {
        self.save_time = lib::time::get_current_ms() as u64 + SAVE_EXPLORE_INTERVAL;
        //TODO
        //保存探索信息
        DbHandler::save_explore_info(self.into())
            .map_err(|e| logthrow!(e, e))
            .ok();
        self.finish_list.sort();
        self.finish_list.dedup();
    }
    pub(crate) fn close(&mut self) -> anyhow::Result<()> {
        self.state = ExploreState::Closed;
        //如果结束时保存标记已设置,不再重新设置
        if !self.saved {
            self.save_explore();
            self.saved = true;
        }
        //info!("explore {:?} closed",(self.player_id,self.explore_id));
        super::super::entry::remove_explore(self.player_id);
        Ok(())
    }
    pub(crate) fn disconnect(&self) -> anyhow::Result<()> {
        self.player_info.send_msg(SessionTransport::disconnect())?;
        Ok(())
    }
    
    async fn pack_player_info(
        &mut self,
        packet: &mut lib::proto::Es2CMsgExploreSync,
    ) -> anyhow::Result<()> {
        if self.player_info.flush_dirty(
            explore_player_dirty_flag::CHARACTER
                | explore_player_dirty_flag::ATTRIBUTE,
        ) {
            let characters = self
                .player_info
                .characters
                .iter()
                .map(|c| lib::proto::ExploreCharacterInfo {
                    id: c.config_id,
                    state: c.state as i32,
                    attributes: c.get_base_attrs(),
                    ..Default::default()
                })
                .collect();
            let mut pack = lib::proto::Es2cMsgExploreCharacters::new();
            pack.set_characters(characters);
            packet.set_characters(pack);
        }
        if self
            .player_info
            .flush_dirty(explore_player_dirty_flag::POSITION)
        {
            let mut locate = lib::proto::Point2::new();
            let cur = self.player_info.position();
            locate.x = cur.x;
            locate.y = cur.y;
            packet.set_locate(locate);
        }
        //新角色入伍,存入数据库
        if self.player_info.new_characters.len() > 0 {
            //TODO 写入数据库在空闲时处理
            DbHandler::save_character(self.player_id, &self.player_info.new_characters).await?;
            self.player_info.new_characters.clear();
        }
        packet.set_food(self.player_info.food as i32);
        packet.set_max_food(self.player_info.max_food);
        packet.set_consumption(self.player_info.consumption);
        Ok(())
    }
    ///包装角色和玩家信息
    async fn pack_sync_msg(
        &mut self,
        packet: &mut lib::proto::Es2CMsgCurrentEventResp,
    ) -> anyhow::Result<()> {
        //判断角色是否还有有效角色 如果无可用角色,探索结束
        if self
            .player_info
            .characters
            .iter()
            .find(|c| c.state == CharacterState::Active)
            .is_none()
        {
            info!("explore {:?} 角色全部重伤,探索失败", self.log_info());
            //探索失败,触发的新事件不再有效
            if packet.has_trigger_event() {
                packet.take_trigger_event();
            }
            self.state = ExploreState::Failed;
        }
        else {
            //TODO
        }
        Ok(())
    }
    ///获取探索结果
    fn handle_explore_result(&mut self) -> Option<ExploreSessionTransport> {
        match self.state {
            ExploreState::Finished => {
                info!("player {} finished explore", self.player_id);
                let mut msg = lib::proto::Es2PsMsgExploreEndSync::new();
                msg.set_result(lib::proto::ExploreResult::FINISHED);
                msg.set_player_id(self.player_id);
                Some(ExploreSessionTransport::new(
                    lib::proto::proto_code::DEFAULT_MAIN_CODE,
                    lib::proto::proto_code::msg_id_es_ps::EXPLORE_END_SYNC,
                    0,
                    Box::new(msg),
                ))
            }
            ExploreState::Failed => {
                info!("player {} failed explore", self.player_id);
                let mut msg = lib::proto::Es2PsMsgExploreEndSync::new();
                msg.set_result(lib::proto::ExploreResult::FAILED);
                msg.set_player_id(self.player_id);
                Some(ExploreSessionTransport::new(
                    lib::proto::proto_code::DEFAULT_MAIN_CODE,
                    lib::proto::proto_code::msg_id_es_ps::EXPLORE_END_SYNC,
                    0,
                    Box::new(msg),
                ))
            }
            _ => None,
        }
    }
    async fn handle_move(&mut self, packet: PackBuffer) -> anyhow::Result<ExploreSessionTransport> {
        let pack = packet
            .unpack::<lib::proto::C2EsMsgExploreMoveReq>()
            .map_err(|_| lib::error::unpack_err())?;
        let pp = pack.get_target().clone();
        let mut target = lib_shared::map::Point2::new(pp.x, pp.y);
        let mut resp = lib::proto::Es2CMsgExploreMoveResp::new();
        let pos = self.player_info.position();
        //如果事件队列不为空,且当前事件未完成,不允许移动
        if !self.event_trigger.empty(){
            //当前事件未完成,不能移动
            resp.set_result(2);
        }
        else if let Some(mut path)= self.map.get_path(pos, target, &|pos_next| self.player_info.visiable_points_local.contains(pos_next), &|p| p == &target){
            let mut sync = lib::proto::Es2CMsgExploreSync::new();
            self.player_info.prev_pos = pos;
            self.map.bind_point(&mut target);
            info!("explore {:?} player {:?} move to {:?}, path {:?}", self.log_info(), pos, target, path);
            //起始点不做寻路路径
            if let Some(index) = path
                .iter()
                .enumerate()
                .find(|(_, x)| x == &&pos)
                .map(|t| t.0)
            {
                path.remove(index);
            }
            //行走距离
            let mut cost_unit = self.move_unit_cost as u32;
            let mut move_cost = self.move_cost as u32;
            if self.player_info.food == 0 {
                move_cost = self.move_cost_hp as u32;
                cost_unit = self.move_unit_cost_hp as u32;
            }
            info!(
                "explore {:?} food {}, cost {:?} - config {:?}",
                self.log_info(),
                self.player_info.food,
                (move_cost, cost_unit),
                (self.move_cost_hp, self.move_unit_cost_hp)
            );
            fn cost_evaluate(player_info: &mut ExplorePlayer, cost: u32) -> bool {
                //食物够的情况下,扣食物
                if player_info.food > 0 {
                    if player_info.food >= cost as i32 {
                        player_info.food -= cost as i32;
                    } else {
                        player_info.food = 0;
                    }
                }
                //食物不够的情况下,扣角色血量
                else {
                    let cost_blood = cost as i32;
                    player_info.cost_health(cost_blood);
                }
                player_info.food > 0
                    || player_info
                        .characters
                        .iter()
                        .find(|c| c.state == CharacterState::Active)
                        .is_some()
            }
            if cost_evaluate(&mut self.player_info, move_cost) {
                for mut point in path {
                    self.player_info.current_step += 1;
                    self.map.bind_point(&mut point);
                    let mut current_event = 0;
                    //更新count值
                    self.player_info.step_count -= 1;
                    //更新位置
                    self.player_info.prev_pos = self.player_info.position();
                    self.player_info.set_position(point);
                    if self.player_info.food == 0 {
                        move_cost = self.move_cost_hp as u32;
                        cost_unit = self.move_unit_cost_hp as u32;
                    }
                    //消耗完毕
                    if !cost_evaluate(&mut self.player_info, cost_unit) {
                        break;
                    }                        
                    self.event_trigger.trigger(&mut self.player_info, &mut self.map, point);
                    //一旦触发了强交互事件就不继续移动
                    if !self.event_trigger.empty(){
                        target = point;
                        break;
                    }
                }
            }
            //移除掉地图外的点id并且发送玩家视野
            resp.set_explored_map(
                self.player_info
                    .visiable_points
                    .iter()
                    .filter(|id| {
                        self.map
                            .iter()
                            .find(|point| point.id() == **id as u16)
                            .is_some()
                    })
                    .map(|f| *f as i32)
                    .collect(),
            );
            self.player_info.visiable_points.clear();
            let event_resp = sync.mut_event_detail();
            self.pack_sync_msg(event_resp)
                .await
                .map_err(|e| logthrow!(e, e))
                .ok();
            self.pack_player_info(&mut sync).await.ok();
            resp.set_explore_info(sync);
            resp.set_result(0);
            let mut p = lib::proto::Point2::new();
            p.x = target.x;
            p.y = target.y;
            resp.set_move_target(p);
        } else {
            info!(
                "player {} move to {:?} fail, path not found",
                self.player_id, target
            );
            resp.set_result(3);
        }
        info!("explore {:?} on move resp {:?}", self.log_info(), resp);
        Ok(SessionTransport::new(
            lib::proto::proto_code::DEFAULT_MAIN_CODE,
            crate::msg_id::EXPLORE_MOVE_RESP,
            packet.header().squence,
            Box::new(resp),
        ))
    }
    async fn handle_battle(
        &mut self,
        packet: PackBuffer,
    ) -> anyhow::Result<ExploreSessionTransport> {
        let pack = packet
            .unpack::<lib::proto::C2EsMsgBattleResultReq>()
            .map_err(|_| lib::error::unpack_err())?;
        info!("explore {:?} battle req {:?}", self.log_info(), pack);
        let mut resp = lib::proto::Es2CMsgBattleResultResp::new();
        if self.event_trigger.empty() {
            resp.set_result(1);
        } else {
            resp.set_result(0);
            //TODO
        }
        info!("explore {:?} battle resp {:?}", self.log_info(), resp);
        Ok(SessionTransport::new(
            lib::proto::proto_code::DEFAULT_MAIN_CODE,
            crate::msg_id::EXPLORE_BATTLE_RESULT_RESP,
            packet.header().squence(),
            Box::new(resp),
        ))
    }
    
    ///加载信息
    pub async fn create_explore(&mut self, time_out: Option<u64>) -> anyhow::Result<()> {
        let info = match time_out {
            Some(tm) if tm > 0 => {
                match tokio::time::timeout(std::time::Duration::from_millis(tm), 
                DbHandler::on_create_explore(self.player_id, self.explore_cfg_id, &self.token, self.player_info.position())).await{
                    Ok(ret) => {
                        info!("loaded explore info {:?}", ret);
                        ret?
                    }
                    Err(err) => {
                        info!("load explore timeout {:?}", err);
                        return Ok(());
                    }
                }
            },
            _ => {                
                DbHandler::on_create_explore(self.player_id, self.explore_cfg_id, &self.token, self.player_info.position()).await?
            }
        };
        self.explore_id = info.id;
        self.player_info.food = info.food;
        let pos = self.player_info.position();
        self.map.bind_point(&mut self.player_info.position_mut());
        self.state = info.state.into();
        Ok(())
    }
    #[inline]
    pub(crate) fn player_id(&self) -> u64 {
        self.player_id
    }
    ///重新激活探索,返回token
    pub fn reconnect(&mut self, server_id: usize) {
        //如果当前玩家正在探索,将当前玩家踢下线
        self.plat_server = server_id;
        let token: u64 = rand::thread_rng().gen();
        self.state = ExploreState::Reconnecting(0);
        self.heart_timer.reset();
        self.save_timer.reset();
        self.token = format!("{:0x}{:0x}", self.explore_id, token);
        info!(
            "explore {:?} reactive token {}",
            self.log_info(),
            self.token
        );
    }


    pub(crate) fn event_handler(
        &mut self,
    ) -> tokio::sync::mpsc::UnboundedSender<SocketMessage<PlayerSessionInfo>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.event_handler = rx.into();
        tx
    }   
}

impl Into<ExploreInfo> for &Explore {
    fn into(self) -> ExploreInfo {
        let mut def = ExploreInfo::default();
        def.id = self.explore_id;
        def.explore_id = self.explore_cfg_id;
        def.player_id = self.player_id;
        def.state = match self.state {
            ExploreState::Exploring | ExploreState::Disconnected(_) => {
                super::db_handler::EXPLORE_STATE_NORMAL
            }
            ExploreState::Finished => super::db_handler::EXPLORE_STATE_FINISHED,
            _ => super::db_handler::EXPLORE_STATE_REMOVED,
        };           
        //TODO GET EVENTS
        def.position = Some(sqlx::types::Json(self.player_info.position()));
        def.food = self.player_info.food as i32;
        def.finished_event = def
            .finished_events
            .iter()
            .filter(|e| e.progress_event == 1)
            .count() as i32;
        def
    }
}
impl Into<ExploreInfo> for &mut Explore {
    fn into(self) -> ExploreInfo {
        let explore: &Explore = self;
        explore.into()
    }
}
#[async_trait]
impl AsyncContextImpl<ExploreBuilder, ()> for Explore {
    fn new(_: ExploreBuilder) -> Self {
        Self::create(0, 0, 0, Default::default(), Default::default(), 0).expect("fail")
    }
    ///receive package
    async fn deal_msg(
        &mut self,
        msg: SocketMessage<()>,
        handler: &mut Option<AsyncSessionHandler<()>>,
    ) -> anyhow::Result<()> {
        if handler.is_none() {
            return lib::error::broken_pipe();
        }
        let handler = handler.as_mut().unwrap();
        match msg {
            //in explore , one contex is one player
            SocketMessage::Message(msg) | SocketMessage::SessionMessage((_, msg)) => {
                //info!("explore {:?} deal_msg {}, handler {:p}", self.log_info(), msg.header().code(),handler);
                self.handle_msg(msg, handler).await?;
            }
            SocketMessage::OnDisconnect => {
                self.state = ExploreState::Disconnected(0);
                self.close()?;
                self.disconnect()?;
            }
            _ => (),
        }
        Ok(())
    }
    ///self async events
    async fn context_check(
        &mut self,
        handler: &mut Option<AsyncSessionHandler<()>>,
    ) -> anyhow::Result<()> {
        if self.event_handler.is_none() {
            error!("call event_handler before start explore context!");
            return lib::error::broken_pipe();
        }
        if self.save_time == 0 {
            self.save_time = lib::time::get_current_ms() as u64 + SAVE_EXPLORE_INTERVAL;
        }
        let mut dura = self.save_time as i64 - lib::time::get_current_ms();
        //100毫秒内直接保存
        if dura <= 100 {
            self.save_explore();
            dura = SAVE_EXPLORE_INTERVAL as i64;
        }
        let state = self.state;
        //当前进行中的事件为空时,才会计算npc逻辑
        destruct_self!(self, event_handler);
        tokio::select! {
            msg = event_handler.as_mut().unwrap().recv() => {
                match msg {
                    //player connect in 
                    Some(SocketMessage::Template(player_info)) => {
                        let player_id = player_info.player_id;
                        let ret = self.append(player_info).await?;
                        if let Some(h) = handler{
                            info!("explore {:?} reconnect, old handler {:p}, new handler {:p}", self.log_info(), h, &ret);
                        }
                        else{
                            info!("explore {:?} connected, old handler None, new handler {:p}", self.log_info(), &ret);
                        }
                        if ret.send(SessionTransport::template(ExploreSharedChannel{ proxy: self.sender.clone().into() })).is_err(){
                            error!("explore {:?} player {} connect fail ", self.log_info(), player_id );
                        }
                        self.player_info.set_handler(ret.into());
                    },
                    Some(SocketMessage::ChannelMessage((channel_id,packet))) => {
                        info!("explore {:?} recv msg {} - {:?}", self.log_info(), channel_id, packet);
                        if packet.header().sub_code() == lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_REQ{
                            let _ = packet.unpack::<lib::proto::Ps2EsMsgExploreReq>().map_err(|_| lib::error::unpack_err())?;
                            self.reconnect(channel_id);
                            let mut resp = lib::proto::Es2PsMsgExploreResp::new();
                            resp.set_result(lib::proto::ExploreCreateResult::SUCCESS);
                            resp.set_explore_uuid(self.get_explore_uuid());
                            resp.set_player_id(self.player_id);
                            resp.set_access_token(self.access_token().to_string());
                            crate::server::channel::channel_service::send_msg(SessionTransport::new(
                                lib::proto::proto_code::DEFAULT_MAIN_CODE,
                                lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_RESP,
                                packet.header().squence(),
                                Box::new(resp)),
                                self.get_plat_server()).map_err(|e| logthrow!(e,e)).ok();
                        } 
                        else if packet.header().sub_code() == lib::proto::proto_code::msg_id_es_ps::FIGHT_SUCCESS_RESP{
                            let pack = packet.unpack::<lib::proto::Ps2EsMsgFightSucessResp>().map_err(|_| lib::error::unpack_err())?;
                            if pack.result == lib::proto::ExploreCreateResult::SUCCESS {
                                for e in pack.exp {
                                    self.player_info.add_exp(e.get_exp(), e.get_config_id());
                                }
                            }
                        }
                        //这里需要处理保存间隔
                    }
                    _ => (),
                }
            },
            _ = tokio::time::sleep_until(tokio::time::Instant::now() + std::time::Duration::from_millis(dura as u64)), if state == ExploreState::Exploring => {                
                self.save_explore();
            }
        }
        Ok(())
    }

    fn on_close(&mut self) {
        //info!("explore {:?} on_close",(self.explore_id,self.player_id));
        self.close().ok();
    }

    #[inline]
    #[allow(unused)]
    fn update_handler(&mut self) -> Option<AsyncSessionHandler<()>> {
        lib::SessionHandler::new(
            self.explore_id as usize,
            lib::SocketHandler::new(self.tsender.clone(), self.receiver.take().unwrap()),
        )
        .into()
    }
}
#[cfg(test)]
#[test]
fn test_timeout() {
    #[derive(Debug)]
    pub struct ExploreInfo {
        pub id: u64,
        pub player_id: u64,
        pub state: i32,
        pub explore_id: u32,
        pub max_event: i32,
        pub finished_event: i32,
        pub create_time: chrono::DateTime<chrono::Local>,
        pub token: String,
        pub events: Vec<crate::server::explore::db_handler::ExploreEventInfo>,
        pub position: Option<sqlx::types::Json<lib_shared::map::Point2>>,
        pub food: i32,
        pub unique_events: Vec<(i32, u32)>,
    }
    impl Default for ExploreInfo {
        fn default() -> Self {
            Self {
                create_time: chrono::Local::now(),
                id: 0,
                player_id: 0,
                state: 0,
                explore_id: 0,
                max_event: 0,
                finished_event: 0,
                token: "".to_string(),
                events: Default::default(),
                position: Default::default(),
                unique_events: Default::default(),
                ..Default::default()
            }
        }
    }
    println!("def {:?}", ExploreInfo::default());
    use tokio::time;
    lib::db::start_pools(vec![lib::db::DbPoolInfo {
        db_path: "mysql://banagame:banagame123@172.16.8.219:3306/bg_db_server".to_string(),
        db_name: "bg_db_server".to_string(),
        max_conn: 20,
    }]);
    println!("load db");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()
        .expect("fail to build runtime");
    runtime.block_on(async {
        println!("start timeout check");
        match time::timeout(std::time::Duration::from_nanos(10), DbHandler::on_create_explore(8225990, 0, "7d84c7705e7c94af72e7e4", Default::default())).await{
            Ok(ret) => {
                println!("ok {:?}", ret);
            }
            Err(e) => {
                println!("err {:?} ", e);
            }
        }
    });
}
