use shared::db::DbCommand;
use shared::proto::PackBuffer;
use shared::{proto::Message, timer::*, SessionTransport, SyncSessionHandler};
use crate::player::*;
use super::world_session_handler;
///max ms time for sessions to keep offline state
pub const MAX_SESSION_IDLE_TIME: u32 = 30*1000;
///max login time for sessions
pub const MAX_SESSION_LOGIN_TIME: u32 = 30*1000;
pub const MAX_SESSION_LOGIN_WAIT_TIME: u32 = 30*1000;
const HEART_CHECK: bool = false;
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WorldSessionState{
    Normal,
    ///wait for login msg
    Login(u32),
    ///loading state with loading time
    Loading(u32),
    ///offline with offline duration
    Offline(u32),
    ///session was kicked off and will be removed at next update
    KickOff,
}
pub struct WorldSession{
    session_id: usize,
    state: WorldSessionState,
    session_handler: world_session_handler::WorldSessionHandler,
    event_timer: IntervalTimer,
    heart_timer: IntervalTimer,
    ///player state of this session
    player: Player,
    kick_off_reason: i32,
}
#[allow(unused)]
impl WorldSession{
    pub fn new(session_id: usize,session: SyncSessionHandler<()>) -> Self{
        Self{
            session_id, state: WorldSessionState::Login(0), 
            event_timer: IntervalTimer::new(60*1000),
            //30s for offline check
            heart_timer: IntervalTimer::new(30*1000),
            player: Player::new((session_id & 0xffffffffffffffff) as u64,session.msg_handler()),
            session_handler: world_session_handler::WorldSessionHandler::new(session),
            kick_off_reason: 0,
        }
    }
    pub fn id(&self) -> usize{self.session_id}
    ///player id of this session
    #[inline]
    pub fn player_id(&self) -> u64{
        self.player.player_id()
    }
    #[inline]
    pub fn get_kick_off_reason(&self) -> i32{ self.kick_off_reason}
    #[inline] 
    pub fn current_state(&self) -> WorldSessionState{
        self.state
    }
    ///send pack to remote
    pub fn send_pack<T: Message>(&self, proto: u16, sub_proto: u16, rpc_squence: u32, msg: T) -> std::io::Result<()>{
        self.session_handler.send(SessionTransport::new(proto, sub_proto, rpc_squence, Box::new(msg))).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
        if proto == crate::msg_id::KICKOFF{
            self.session_handler.send(SessionTransport::disconnect()).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
        }
        Ok(())
    }
    ///check if session is alive(offline if also alive unless it's duration is larger than idle time)
    #[inline]
    pub fn alive(&self) -> bool{
        match self.state {
            WorldSessionState::KickOff => false,
            WorldSessionState::Login(tm) if tm < MAX_SESSION_LOGIN_WAIT_TIME => false,
            WorldSessionState::Loading(tm) if tm < MAX_SESSION_LOGIN_TIME => false,
            WorldSessionState::Offline(dur) if dur < MAX_SESSION_IDLE_TIME => false,
            _ => true,
        }
    }
    ///session is offline state
    #[inline]
    pub fn offline(&self) -> bool{
        match self.state{
            WorldSessionState::Offline(_) => true,
            _ => false,
        }
    }
    ///on session lost disconnection with client
    #[inline]
    pub fn on_disconnected(&mut self){
        self.state = WorldSessionState::Offline(0);
    }
    ///on session reconnected with client
    #[inline]
    pub fn on_reconnected(&mut self){
        self.state = WorldSessionState::Normal;
    }
    pub(crate) fn reset_heart_timer(&mut self){
        self.heart_timer.reset();
        if self.offline(){
            self.on_reconnected();
        }
    }
    pub fn update(&mut self, diff: u32){
        //kicked player will not update anymore
        if self.state == WorldSessionState::KickOff{
            return;
        }
        self.update_db_handler();
        while let Some(event)= self.session_handler.deal_msg(){
            match event{
                world_session_handler::WorldSessionHandlerEvent::OnMessage(msg) => {
                    self.reset_heart_timer();
                    self.on_msg(msg);
                },
                world_session_handler::WorldSessionHandlerEvent::Disconnected => {
                    info!("session {:?} disconnected for deal_msg fail", (self.session_id, self.player_id()));
                    self.on_disconnected();
                },
                _ => (),
            }
        }
        match self.state{
            WorldSessionState::Login(time)   => {
                if time < MAX_SESSION_LOGIN_WAIT_TIME{
                    self.state = WorldSessionState::Login(time + diff);
                }
                else{
                    self.state = WorldSessionState::KickOff;
                    self.kick_off_reason = 2;
                    info!("player {} kick off for login timeout ", self.player_id());
                }
            },
            WorldSessionState::Loading(time)  => {
                if time < MAX_SESSION_LOGIN_TIME{
                    let loading_time = time + diff;
                    self.state = WorldSessionState::Loading(loading_time);     
                    //TODO
                }  
                else{
                    self.state = WorldSessionState::KickOff;
                    self.kick_off_reason = 1;
                    info!("player {} kick off for loading timeout ", self.player_id());
                }     
            },
            WorldSessionState::Offline(time)  => {
                if time < MAX_SESSION_IDLE_TIME{
                    let off_time = time + diff;
                    self.state = WorldSessionState::Offline(off_time);
                }
                else{
                    self.state = WorldSessionState::KickOff;
                    self.kick_off_reason = 3;
                    info!("player {} kick off for offline timeout ", self.player_id());
                }
            }
            WorldSessionState::Normal => {
                //TODO
                self.event_timer.update(diff as i64);
                if self.event_timer.passed(){
                    self.event_timer.reset();
                }
                self.heart_timer.update(diff as i64);
                //heartbeat time out,connection set to disconnected
                if self.heart_timer.passed() && HEART_CHECK{
                    self.on_disconnected();
                }
                self.player.update(diff as i64);
            }
            _ => {

            }
        }
    }
    fn on_msg(&mut self, msg: PackBuffer){
        let header = msg.header();
        let code = header.sub_code();
        if code != shared::proto::proto_code::HEART {
            info!("world session received msg {}",header.sub_code());
        }
        match code{
            shared::proto::proto_code::HEART => {
                self.reset_heart_timer();
            },
            crate::msg_id::MSG_LOGIN => {
                match msg.unpack::<shared::proto::C2PMsgLogin>(){
                    Ok(login) if login.account.len() > 1 => {
                        self.state = WorldSessionState::Loading(0);
                        info!("Player {} recv login message [{}] access [{}]",self.session_id,login.get_account(), login.get_access_token());
                        let cmd = if_else!(header.squence > 0,DbCommand::rpc_default(header.squence),DbCommand::normal_default());
                        if let Err(e) = self.player.db_handler().load_player_info(login.get_account(), login.get_access_token(),cmd){
                            self.kick_off_reason = 4;
                            self.state  = WorldSessionState::KickOff;
                            error!("player {} kick off by loading player info fail! {}",login.get_account(),e);
                        }
                    },
                    Ok(_) => {
                        info!("Session {} recv error login message (illegal account length), kick off",self.session_id);
                        self.kick_off_reason = 5;
                        self.state  = WorldSessionState::KickOff;
                    },
                    Err(e) => {
                        info!("Session {} recv unrecognized login message {}, kick off",self.session_id,e);
                        self.kick_off_reason = 4;
                        self.state  = WorldSessionState::KickOff;
                    },
                }
            },
            crate::msg_id::LOGIN_SUCCESS => {
                match msg.unpack::<shared::proto::C2PMsgLoginSuccess>(){
                    Ok(_) => {
                        self.state = WorldSessionState::Normal;
                        self.heart_timer.reset();
                    },
                    Err(e)=> {
                        self.kick_off_reason = 4;
                        self.state  = WorldSessionState::KickOff;
                        info!("Player {} recv unrecognized login message {}, kick off",self.player.get_name(),e);
                    }
                }
            }
            opcode => {
                if let Err(e) = self.player.on_msg(msg){
                    self.state  = WorldSessionState::KickOff;
                    self.kick_off_reason = 4;
                    info!("Session {} recv unrecognized message {}, kick off {:?}",self.session_id,opcode,e);
                }
                else{
                    self.heart_timer.reset();
                }
            }
        }
    }

    fn update_db_handler(&mut self) -> () {
        match self.state{
            WorldSessionState::Loading(time) => {
                if let Ok(result)= self.player.db_handler().try_get_player_info(){
                    let mut resp = shared::proto::P2CMsgLoginResp::new();
                    let rpc = match shared::db::unwrap_cmd(result).flat(){
                        Ok((rpc,player)) => {        
                            info!("player [{}] load success after {} mills",self.player_id(),time);                    
                            self.state = WorldSessionState::Normal;
                            self.player.init(player.into());
                            resp.set_result(shared::proto::ELoginRetResp::RR_SUCCESS);
                            resp.set_playerId(self.player.player_id());
                            resp.set_characters(self.player.get_characters().map(|list|{
                                list.iter().map(|cha|{
                                    let mut c = shared::proto::P2cMsgLoginCharacter::new();
                                    c.set_uuid(cha.id);
                                    c.set_own_type(cha.own_type);
                                    c.set_role_id(cha.role_id);
                                    c.set_state(cha.state);
                                    c
                                }).collect()
                            }).unwrap_or_default());
                            rpc
                        },
                        Err(e) => {
                            self.kick_off_reason = 4;
                            self.state = WorldSessionState::KickOff;
                            resp.set_result(shared::proto::ELoginRetResp::RR_ERROR);
                            info!("load player fail, no data found");
                            e
                        }
                    };
                    if let Err(_) = self.session_handler.send(SessionTransport::new(shared::proto::proto_code::DEFAULT_MAIN_CODE,crate::msg_id::MSG_LOGIN_RESP, rpc, Box::new(resp))){
                        info!("Player {} load success, but response fail", self.player.get_name());
                        self.state = WorldSessionState::KickOff;
                    }
                }
            },
            _ => (),
        }
    }
    pub fn on_explore_create_resp(&mut self, resp: shared::proto::Es2PsMsgExploreResp)  -> anyhow::Result<()>{
        self.player.on_explore_create_resp(resp)
    } 
}