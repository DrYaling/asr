//! 探索玩家信息
#![allow(unused)]
use std::collections::{BTreeMap, HashMap};
use std::ops::BitAnd;
use lib::{AsyncSessionHandler, SessionTransport};
use lib::{proto::EExploreEventType};
use lib_shared::attribute::{AttributeBinder, EAttributeType};
use lib_shared::map::Point2;
use chrono::prelude::*;
use super::{ExploreSharedChannel};
use super::explore_event::{ExploreEvent, GameEventState, EventInfo};
///探索玩家更新标志
pub mod explore_player_dirty_flag{
    pub const ATTRIBUTE: u32    = 0x1;
    pub const CHARACTER: u32    = 0x2;
    pub const POSITION: u32     = 0x10;
    pub const ALL: u32          = u32::MAX;
}
#[derive(Debug, Default)]
pub struct ExplorePlayer{
    player_id: u64,
    ///位置
    position: lib_shared::map::Point2,
    ///食物
    pub food: i32,
    pub max_food: u32,
    ///食物总消耗
    pub consumption: u32,
    pub characters: Vec<ExploreCharacter>,
    ///新加入队伍的角色
    pub new_characters: Vec<u32>,
    ///当前行走步数
    pub current_step: u32,
    pub visiable_points: Vec<i32>, /* 存储临时的视野区域点 */
    pub visiable_points_local: Vec<Point2>,/* 存储可视点的坐标 */
    ///计算玩家步数
    pub step_count: i32,
    ///上一个移动位置点
    pub prev_pos: Point2,
    pub origin_pos: Point2,
    session_handler: Option<AsyncSessionHandler<ExploreSharedChannel>>,
    pub gm_authority: u32,   //是否有GM权限
    pub dirty_flag: u32,
    speed: u32,
    fov: u32,
    pub trigger_enabled: bool,
}
impl ExplorePlayer{
    pub fn new(player_id: u64, map_id: u32, pos: Point2, food: u32, characters: &Vec<u32>, gm_authority: u32) ->Self{ 
        let mut info = Self::default();
        info.dirty_flag = explore_player_dirty_flag::ALL;
        info.position = pos;
        info.food = food as i32;
        info.player_id = player_id;
        info.trigger_enabled = true;
        info.fov = lib::libconfig::common::get_value("DisperseFog").unwrap_or(4);
        info.speed = lib::libconfig::common::get_value("MovementSpeed").unwrap_or(10);
        let configs = lib::libconfig::partner_config::load_partner_configs(|(id,_)| characters.contains(*id));
        info.characters= characters.iter().map(|id|{
            let config = configs.get(id);
            let mut attr = AttributeBinder::default();
            let health = config.map(|t| t.iHp as u32).unwrap_or(1000);
            let atk = config.map(|t| t.iAttack).unwrap_or(100);
            let def = config.map(|t| t.iDefence).unwrap_or(100);
            let san = config.map(|t| t.san).unwrap_or(100);
            attr.set_attr(EAttributeType::MaxHealth, health as i32);
            attr.set_attr(EAttributeType::Health, health as i32);
            attr.set_attr(EAttributeType::Attack, atk);
            attr.set_attr(EAttributeType::Defense, def);
            ExploreCharacter{
                config_id: *id,
                state: CharacterState::Active,
                attribute_binder: attr,
                ..Default::default()
            }
        }).collect();
        info.gm_authority = gm_authority;
        info.max_food = lib::libconfig::common::get_value("DefaultFood").unwrap_or(100);

        info
    }
    //set player session handler
    #[inline]
    pub fn set_handler(&mut self, handler: Option<AsyncSessionHandler<ExploreSharedChannel>>){
        self.session_handler = handler;
    }
    #[inline]
    pub fn set_fov(&mut self, fov: u32) { self.fov = fov; }
    pub fn send_msg(&self, msg: SessionTransport<()>)-> anyhow::Result<()>{
        match self.session_handler.as_ref(){
            Some(handler) => {
                handler.send(msg.into()?)
            },
            None => {
                error!("player {} disconnected", self.player_id);
                lib::error::broken_pipe()
            }
        }
    }
    pub fn set_position(&mut self, pos: Point2){
        self.position = pos;
        self.dirty_flag |= explore_player_dirty_flag::POSITION;
    }
    ///位置信息
    #[inline]
    pub fn position_mut(&mut self) -> &mut Point2{ &mut self.position }
    ///团队扣血
    pub fn cost_health(&mut self, cost: i32){
        self.characters.iter_mut().for_each(|cha|{
            if cha.state != CharacterState::Active{
                return;
            }
            let current_hp = cha.max_health() as i32;
            let new_hp = 1.max(current_hp - cost);
            cha.set_base_attr(EAttributeType::MaxHealth, new_hp);
            if new_hp <= 1{
                cha.state = CharacterState::Injured;
            }
        });
        self.dirty_flag |= explore_player_dirty_flag::ATTRIBUTE;
    }
    ///设置某个角色的属性
    pub fn set_base_attr(&mut self, character: u32, attr: EAttributeType, value: i32){
        if let Some(cha) = self.characters.iter_mut().find(|c| c.config_id == character){
            cha.set_base_attr(attr, value);
            if attr == EAttributeType::Health && value <= 1{
                cha.state = CharacterState::Injured;
            }
            self.dirty_flag |= explore_player_dirty_flag::ATTRIBUTE;
        }
    }
    ///更新dirty flag,并将该位设置为0
    /// flag必须是explore_player_dirty_flag中的一个,否则会更新错误
    pub fn flush_dirty(&mut self, flag: u32) -> bool{
        //assert!(flag.is_power_of_two(), "flag {} is not within explore_player_dirty_flag", flag);
        let flag = self.dirty_flag & flag;
        self.dirty_flag &= !flag;
        flag != 0
    }
    #[inline]
    ///判断flag是否为true
    pub fn is_dirty(&mut self, flag: u32) -> bool{
        //assert!(flag.is_power_of_two(), "flag {} is not within explore_player_dirty_flag", flag);
        (self.dirty_flag & flag) != 0
    }
    #[inline]
    pub fn set_dirty(&mut self, flag: u32){
        self.dirty_flag |= flag;
    }
    pub fn exp(&self, character: u32) -> i32 {
        self.characters.iter().find(|c| c.config_id == character).map(|c| c.get_exp()).unwrap_or_default()
    }
    pub fn add_exp(&mut self, exp: i32, character: u32) {
        self.characters.iter_mut().find(|c| c.config_id == character).unwrap().add_exp(exp);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterState{
    ///活跃
    Active = 0,
    ///重伤
    Injured = 1,
    ///不可用
    Unusable = 2,
}
impl Default for CharacterState{    
    fn default() -> Self { CharacterState::Active }
}
#[derive(Debug,Clone,Default)]
pub struct ExploreCharacter{
    pub config_id: u32,
    pub state: CharacterState,
    attribute_binder: AttributeBinder,
    exp: i32,
}
impl ExploreCharacter{
    #[inline]
    pub fn get_base_attrs(&self) -> ::protobuf::RepeatedField<lib::proto::CharacterAttribute> {
        self.attribute_binder.iter().enumerate().filter_map(|(idx, value)| {
            let value = *value;
            Some(lib::proto::CharacterAttribute{ attribute_type: idx as i32, value: value, ..Default::default()})
        }).collect()
    }
    #[inline]
    ///生命值,重伤状态为1
    pub fn health(&self) -> u32{ 
        self.attribute_binder.get_attr(EAttributeType::Health) as u32
    }
    #[inline]
    pub fn max_health(&self) -> u32{
        self.attribute_binder.get_attr(EAttributeType::MaxHealth) as u32
    }    
    #[inline]
    pub fn attack(&self) -> u32{
        self.attribute_binder.get_attr(EAttributeType::Attack) as u32
    }
    ///获取属性
    #[inline]
    pub fn get_attr(&self, attr: EAttributeType) -> i32{
        self.attribute_binder.get_attr(attr)
    }
    ///设置基础属性
    #[inline]
    fn set_base_attr(&mut self, attr: EAttributeType, mut value: i32){
        if attr == EAttributeType::Health{
            value = value.min(self.get_attr(EAttributeType::MaxHealth)).max(1);
        }
        //血量上线修改需要修改当前血量
        else if attr == EAttributeType::MaxHealth{
            value = value.max(1);
            //血量同步
            let chp =self.health() as i32;
            let hp = chp.min(value);
            if chp != hp{
                self.attribute_binder.set_attr(EAttributeType::Health, hp);
            }
        }
        self.attribute_binder.set_attr(attr, value)
    }
   
    #[inline]
    pub fn get_exp(&self) -> i32 { self.exp }
    #[inline]
    pub fn add_exp(&mut self, exp: i32) { self.exp += exp; }
}
impl ExplorePlayer{
    #[inline]
    pub fn active(&self) -> bool {
        true
    }
    #[inline]
    pub fn get_children(&self) -> Vec<&ExploreCharacter> {
        self.characters.iter().collect()
    }
    #[inline]
    pub fn left_vitality(&self) -> u32{ 
        self.food as u32 + self.characters.iter()
            .filter(|c| c.active()).fold(0u32, |r,x| r + x.health())
    }
    #[inline]
    pub fn fov(&self) -> u32 { self.fov }
    #[inline]
    pub fn position(&self) -> Point2 { self.position }
    #[inline]
    pub fn step(&self) -> u32 { self.current_step }
    pub fn on_event(&mut self, event: EventInfo){
        if event.state == GameEventState::Finished{            
            //TODO
        }
    }

    /*fn get_visiable_points(&self) -> &Vec<i32> {
        &self.visiable_points
    }*/
    #[inline]
    fn add_visiable_point(&mut self, pos: Point2){
        if self.visiable_points_local.iter().find(| p| p.id() as i32 == pos.id() as i32).is_none(){
            self.visiable_points.push(pos.id() as i32); //加入点id
            self.visiable_points_local.push(pos);   //加入位置坐标
        }
    }
    ///切换暗雷触发器
    #[inline]
    fn switch_trigger(&mut self, enabled: bool) {
        self.trigger_enabled = enabled;
    }
    ///暗雷触发开关
    #[inline]
    fn trigger_enabled(&self) -> bool { self.trigger_enabled }
}
impl ExploreCharacter{
    #[inline]
    pub fn active(&self) -> bool {
        self.state == CharacterState::Active
    }
    #[inline]
    pub fn get_config_id(&self) -> u32 {
        self.config_id
    }
}