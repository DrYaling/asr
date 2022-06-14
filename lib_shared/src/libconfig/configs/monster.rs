//! Monster 配置.
//! 自动生成代码,请勿修改.
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused)]
use serde::{Deserialize};
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MonsterConfig{
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub iModelId: i32,
    #[serde(default)]
    pub iScale: i32,
    #[serde(default)]
    pub monsterAi: i32,
    #[serde(default)]
    pub iVolume: i32,
    #[serde(default)]
    pub iMonsterType: i32,
    #[serde(default)]
    pub iOffsetHue: i32,
    #[serde(default)]
    pub iQuality: i32,
    #[serde(default)]
    pub iCharacteristic: i32,
    #[serde(default)]
    pub iCamp: i32,
    #[serde(default)]
    pub iSexuality: i32,
    #[serde(default)]
    pub iBornSkill: Vec<u32>,
    #[serde(default)]
    pub iChainSkill: u32,
    #[serde(default)]
    pub iPassiveSkill: Vec<u32>,
    #[serde(default)]
    pub iInitAnger: i32,
    #[serde(default)]
    pub iAngerGrowth: i32,
    #[serde(default)]
    pub iBaseMoveBlock: i32,
    #[serde(default)]
    pub iLv: i32,
    #[serde(default)]
    pub iHp: i32,
    #[serde(default)]
    pub iAttack: i32,
    #[serde(default)]
    pub iDefence: i32,
    #[serde(default)]
    pub iCriticalStrick: i32,
    #[serde(default)]
    pub iToughness: i32,
    #[serde(default)]
    pub iReinforcementsDialogueOrder: i32,
    #[serde(default)]
    pub iBattleStartDialogueOrder: i32,
}
impl super::IConfig for MonsterConfig{
    #[inline]
    fn id(&self) -> u32 { self.id as u32 }
}
