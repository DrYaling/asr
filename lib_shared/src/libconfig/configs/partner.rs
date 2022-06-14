//! Partner 配置.
//! 自动生成代码,请勿修改.
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused)]
use serde::{Deserialize};
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartnerConfig{
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub kResPath: String,
    #[serde(default)]
    pub iScale: i32,
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
    pub iBornPassive: Vec<u32>,
    #[serde(default)]
    pub iChangeSkill: Vec<u32>,
    #[serde(default)]
    pub kWeaponTypes: String,
    #[serde(default)]
    pub iSpecialResources: i32,
    #[serde(default)]
    pub iInitAnger: i32,
    #[serde(default)]
    pub iAngerGrowth: i32,
    #[serde(default)]
    pub iBaseMoveBlock: i32,
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
    pub configfix: i32,
    #[serde(default)]
    pub san: i32,
    #[serde(default)]
    pub discovery_skills: Vec<i32>,
    #[serde(default)]
    pub title: i32,
    #[serde(default)]
    pub san_damping: Vec<i32>,
    #[serde(default)]
    pub food_consumed: Vec<i32>,
    #[serde(default)]
    pub recovery_rate: Vec<i32>,
    #[serde(default)]
    pub place: Vec<i32>,
    #[serde(default)]
    pub AngleOfView : Vec<i32>,
}
impl super::IConfig for PartnerConfig{
    #[inline]
    fn id(&self) -> u32 { self.id as u32 }
}
