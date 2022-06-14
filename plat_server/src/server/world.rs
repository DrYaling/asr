//! world
//! 
use lib::{
    SyncSessionHandler, 
    proto::{PackBuffer}, 
    SessionId, 
    timer::*
};
use super::world_session::{WorldSession, WorldSessionState};
use std::collections::BTreeMap;
use crossbeam::{channel::Receiver};
use lib::proto;
pub struct WorldBuilder;
impl WorldBuilder{
    pub fn new()-> Self{
        Self
    }
    pub fn build(self) -> anyhow::Result<World>{
        let (tx,rx) = crossbeam::channel::unbounded();
        Ok(World{
            session_map: Default::default(),
            queued_sessions: Default::default(),
            ///player session map stores account id and player session id
            player_sessions: Default::default(),
            ///periodic timer trigger
            // periodic_timer: Vec<PeriodicTimer>,
            ///interval timers
            timers: Default::default(),
            command_receiver: rx,
            command_sender: tx,
        })
    }
}
///world command eunm
pub enum WorldCommand{
    ///探索服消息
    ExploreMsg(PackBuffer),
}
///daily task timer index
const WORLD_TIMER_DAILY: usize = 0;
const WORLD_EVENT_TIMER: usize = 1;
const WORLD_EVENT_INTERVAL: i64 = 6*60*60*1000;
pub struct World{
    ///world sessions
    session_map: BTreeMap<SessionId,WorldSession>,
    queued_sessions: Vec<WorldSession>,
    ///player session map stores account id and player session id
    player_sessions: BTreeMap<u64,SessionId>,
    ///periodic timer trigger
    // periodic_timer: Vec<PeriodicTimer>,
    ///interval timers
    timers: Vec<IntervalTimer>,
    command_receiver: Receiver<WorldCommand>,
    command_sender: crossbeam::channel::Sender<WorldCommand>,
}
#[allow(unused_variables)]
impl World{
    pub(crate) fn start(&mut self) -> Result<(),std::io::Error>{
        //world event just trigger 6 hours after server restart
        self.timers.push(IntervalTimer::new(WORLD_EVENT_INTERVAL));
        let mut daily = IntervalTimer::new(lib_shared::one_day_time());
        //daily reset at 4:00
        const DAILY_RESET_TIME: i64 = 4*60*60*1000;
        let mut time_stamp = lib_shared::get_timestamp_of_today();
        let diff = time_stamp - DAILY_RESET_TIME;
        //set current time
        if diff > 0{
            time_stamp = diff;
        }
        else{
            time_stamp = lib_shared::one_day_time() + diff;
        }
        daily.set_current(time_stamp);
        self.timers.push(daily);

        Ok(())
    }
    #[inline]
    pub fn world_cmd_handler(&self) -> crossbeam::channel::Sender<WorldCommand>{
        self.command_sender.clone()
    }
    ///add new session to world
    pub fn add_session(&mut self, session: SyncSessionHandler<()>){
        //TODO
        self.queued_sessions.push(WorldSession::new(session.id(), session));
    }
    ///add active session to world
    /// 
    /// if same session exist, then kickoff the old one
    fn add_active_session(&mut self,session: WorldSession){
        match self.session_map.iter().find(|(_,s)| s.player_id() == session.player_id()).map(|t| *t.0){
            Some(session_id) => {
                self.kick_off_session(session_id).ok();
            },
            None => {
            }
        }
        self.player_sessions.insert(session.player_id(), session.id());
        self.session_map.insert(session.id(), session);
    }
    ///kick off player and remove this session
    fn kick_off_session(&mut self, session: SessionId) -> std::io::Result<()>{
        match self.session_map.remove(&session){
            Some(session) => {
                self.player_sessions.remove(&session.player_id());
                let mut pack = proto::P2CMsgKickOff::new();
                pack.set_reason(session.get_kick_off_reason());
                info!("kick off player [{}] (session [{}]-state [{:?}]) for reason {}",session.player_id(),session.id(),session.current_state(),session.get_kick_off_reason());
                session.send_pack(lib::proto::proto_code::DEFAULT_MAIN_CODE,crate::msg_id::KICKOFF, 0, pack)
            },
            None => Err(std::io::ErrorKind::NotConnected.into()),
        }
    }
    ///kick off a session
    fn kick_off_session1(session: WorldSession) -> std::io::Result<()>{
        let mut pack = lib::proto::P2CMsgKickOff::new();
        pack.set_reason(1);
        session.send_pack(lib::proto::proto_code::DEFAULT_MAIN_CODE,crate::msg_id::KICKOFF,0, pack)
    }
    #[inline]
    fn recv_cmd(&self) -> Option<WorldCommand>{
        self.command_receiver.try_recv().ok()
    }
    ///update world 
    pub(crate) fn update(&mut self, diff: u32){
        self.handle_cmd();
        //timers update
        self.timers.iter_mut().for_each(|timer| timer.update(diff as i64));
        if self.timers[WORLD_EVENT_TIMER].passed(){
            //TDOO
        }
        if self.timers[WORLD_TIMER_DAILY].passed(){
            //TDOO
        }
        
        //recv command
        // while let Some(cmd) = self.recv_cmd(){
        //     match cmd{
        //     }
        // }
        let mut activated_sessions = Vec::new();
        //check queued sessions
        for session in self.queued_sessions.iter_mut(){
            session.update(diff);
            if session.current_state() == WorldSessionState::Normal || session.current_state() == WorldSessionState::KickOff{
                activated_sessions.push(session.id());
            }
        }
        //move or merge active sessions to world session map
        for active_session in activated_sessions{
            match self.queued_sessions.iter().position(|s| s.id() == active_session){
                Some(idx) => {
                    let session = self.queued_sessions.remove(idx);
                    //load success, add to session map
                    if session.current_state() == WorldSessionState::Normal{
                        self.add_active_session(session);
                    }
                    //load fail,kick off it
                    else{
                        self.player_sessions.remove(&session.player_id());
                        Self::kick_off_session1(session).ok();
                    }
                },
                //unreachable code
                _ => (),
            }
        }
        //kick off dead sessions
        let dead_sessions = self.session_map.iter().filter(|(_,session)|{
            !session.alive()
        }).map(|(_,s)| s.id()).collect::<Vec<_>>();
        for session in dead_sessions{
            self.kick_off_session(session).ok();
        }
        //session update
        self.session_map.iter_mut().for_each(|(_,session)| session.update(diff));
    }
    fn handle_cmd(&mut self){
        while let  Some(cmd) = self.recv_cmd() {
            match cmd{
                WorldCommand::ExploreMsg(packet) => {
                    if let Err(e) = self.handle_explore_channel_msg(packet){
                        error!("recv error packet from explore_channel {:?}",e);
                    }
                },
                //_ => (),
            }
        }
    }
    ///处理探索服消息
    fn handle_explore_channel_msg(&mut self, packet: PackBuffer) -> anyhow::Result<()> {
        let header = packet.header();
        match header.sub_code() as u16{
            lib::proto::proto_code::msg_id_es_ps::CREATE_EXPLORE_RESP => {
                let pack = packet.unpack::<lib::proto::Es2PsMsgExploreResp>()
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                let player_id = pack.get_player_id();
                if let Some(session) = self.player_sessions.get(&player_id){
                    if let Some(session) = self.session_map.get_mut(session){
                        session.on_explore_create_resp(pack)?;
                    }
                    else{
                        warn!("player disconnected before explore started {}",player_id);
                    }
                }
                else{
                    warn!("player disconnected before explore started {}",player_id);
                }
            }
            opcode => {
                error!("unexpected opcode from explore channel {}",opcode);                
            }
        }
        Ok(())
    }
}