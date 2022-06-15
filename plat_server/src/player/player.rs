//! player 
//! 
use std::collections::VecDeque;

use shared::{Transporter, MsgSendHandler, proto::{self, PackBuffer}, SessionTransport, libconfig::config};

use super::DbHandler;

#[derive(Debug,Copy, Clone, PartialEq, Eq)]
pub struct ExploreReq{
    pub time: i64,
    pub rpc : u32,
}
///玩家操作指令
#[derive(Debug,Copy, Clone, PartialEq, Eq)]
pub enum PlayerOperation{
    ///请求探索,参数为rpc编号
    CreateExplore(ExploreReq),
}
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct CharacterLoader{
    pub id: u64,
    pub role_id: u32,
    pub own_type:i32,
    pub state: i32,
}
#[derive(Debug, Clone)]
pub struct PlayerLoginInfo{
    pub player_id: u64,
    pub name: String,
    pub characters: Vec<CharacterLoader>,
}
impl PlayerLoginInfo{
    pub fn new(player_id: u64, name: String, characters: Vec<CharacterLoader>) -> Self{
        Self{player_id, name, characters}
    }
}
#[derive(Debug)]
pub struct Player{
    guid: u64,
    ///account id of player
    player_id: u64,
    player_info: Option<PlayerLoginInfo>,
    ///操作列表
    operation_queue: VecDeque<PlayerOperation>,
    db_handler: DbHandler,
    msg_handler: MsgSendHandler<Transporter<()>,()>,
}
#[allow(unused)]
impl Player{
    pub fn new(guid: u64, msg_handler: MsgSendHandler<Transporter<()>, ()>) -> Self {
        Self{
            guid,
            player_id: 0,
            player_info: None,
            operation_queue: VecDeque::default(),
            db_handler: DbHandler::default(), 
            msg_handler
        }
    }
    pub fn init(&mut self, data: Option<PlayerLoginInfo>) -> Result<(),usize>{
        self.player_info = data;
        self.player_id = self.player_info.as_ref().map(|info| info.player_id).unwrap_or_default();
        Ok(())
    }
    #[inline]
    pub fn player_id(&self) -> u64{
        self.player_id
    }
    #[inline]
    pub fn get_name(&self) -> &str{
        self.player_info.as_ref().map(|info| info.name.as_str()).unwrap_or_default()
    }
    #[inline]
    pub fn get_characters(&self) -> Option<&Vec<CharacterLoader>>{
        self.player_info.as_ref().map(|info| &info.characters)
    }
    #[inline]
    pub fn db_handler(&mut self) ->&mut DbHandler {
        &mut self.db_handler
    }
    pub fn update(&mut self, diff: i64){        
        let now = chrono::Local::now().timestamp_millis();
        let dead = self.operation_queue.iter_mut().filter(|op|{
            match op{
                PlayerOperation::CreateExplore(req) if req.time + 5_000 < now => true,
                _ => false,
            }
        }).map(|t| t.clone()).collect::<Vec<_>>();
        for op in dead {
            if let Some(idx) = self.operation_queue.iter().enumerate().find(|(_,x)| x == &&op ).map(|t| t.0){
                self.operation_queue.remove(idx);
            }
        }
    }
    fn on_operation_timeout(&mut self, op: PlayerOperation){
        match op{
            PlayerOperation::CreateExplore(req) => {
                info!("player {} create_explore fail, explore req timeout",self.get_name());
                let mut create_req = proto::P2CMsgCreateExploreResp::new();
                create_req.set_result(proto::CreateExploreReqResult::FAIL);
                self.msg_handler.send(SessionTransport::new(
                    proto::proto_code::DEFAULT_MAIN_CODE,
                    crate::msg_id::CREATE_EXPLORE_REQ_RESULT, 
                    req.rpc,
                    Box::new(create_req))).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe)).ok();
            }
        }
    }
    pub fn on_msg(&mut self, msg: PackBuffer) -> anyhow::Result<()> {
        let rpc = msg.header().squence;
        match msg.header().code as u16{
            crate::msg_id::CREATE_EXPLORE_REQ => {
                //如果当前正在探索,直接返回错误
                if self.operation_queue.iter()
                .find(|op| match op {
                    PlayerOperation::CreateExplore(_) => true,
                    _ => false,
                }).is_some(){
                    info!("player {} create_explore fail, explore is creating",self.get_name());
                    let mut create_req = proto::P2CMsgCreateExploreResp::new();
                    create_req.set_result(proto::CreateExploreReqResult::FAIL);
                    self.msg_handler.send(SessionTransport::new(
                        proto::proto_code::DEFAULT_MAIN_CODE,
                        crate::msg_id::CREATE_EXPLORE_REQ_RESULT, 
                        rpc,
                        Box::new(create_req))).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
                }
                else{
                    match msg.unpack::<proto::C2PMsgCreateExploreReq>(){
                        Ok(pack) => {
                            let mut req = proto::Ps2EsMsgExploreReq::new();
                            let characters = shared::server::worker::block_on(DbHandler::load_characters(self.player_id))?;
                            req.set_characters(characters.iter().map(|info| info.role_id).collect());
                            req.set_explore_id(1);
                            req.set_plat_server_id(1);
                            req.set_player_id(self.player_id());
                            crate::server::channel::explore_manager::send_msg(SessionTransport::new(
                                proto::proto_code::DEFAULT_MAIN_CODE,
                                proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_REQ, 0, Box::new(req)))?;
                            self.operation_queue.push_back(PlayerOperation::CreateExplore(ExploreReq{rpc,time: chrono::Local::now().timestamp_millis()}));

                        },
                        Err(e)=> {
                            info!("Player {} recv unrecognized CREATE_EXPLORE_REQ message {}, kick off",self.get_name(),e);
                        }
                    }
                }
            }
            _ => {
                let e = Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted));
                e?;
            }
        }
        Ok(())
    }

    pub(crate) fn on_explore_create_resp(&mut self, resp: proto::Es2PsMsgExploreResp) -> anyhow::Result<()> {
        info!("on_explore_create_resp, {:?}",resp);
        if let Some(op) =  self.operation_queue.iter().enumerate()
        .find(|op| match op.1 {
            PlayerOperation::CreateExplore(_) => true,
            _ => false,
        }).map(|(idx,info)| (idx,info.clone())){
            let mut create_req = proto::P2CMsgCreateExploreResp::new();
            match resp.get_result(){
                proto::ExploreCreateResult::SUCCESS => {
                    create_req.set_result(proto::CreateExploreReqResult::SUCCESS);
                    create_req.set_explore_uuid(resp.get_explore_uuid());
                    create_req.set_access_token(resp.access_token);
                    create_req.set_server_ip(config::get_str("explore_server_ip").unwrap_or_default());
                    create_req.set_server_port(config::get("explore_server_port").unwrap_or_default());
                },
                proto::ExploreCreateResult::FAIL => {
                    create_req.set_result(proto::CreateExploreReqResult::FAIL);
                },
            }
            let rpc = match op.1 {
                PlayerOperation::CreateExplore(r) => r.rpc,
                _ => 0,
            };
            self.msg_handler.send(SessionTransport::new(
                proto::proto_code::DEFAULT_MAIN_CODE,
                crate::msg_id::CREATE_EXPLORE_REQ_RESULT, 
                rpc,
                Box::new(create_req))).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
            self.operation_queue.remove(op.0);
            Ok(())
        }
        else{
            error!("fail to handle create explore for player {} (operation cancled)",self.get_name());
            Ok(())
        }
    }
}