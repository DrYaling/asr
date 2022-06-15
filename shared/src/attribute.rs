//! 属性控制器
use std::collections::BTreeMap;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum EAttributeType{
    None = 0,

    /// 攻击
    Attack = 1,
    /// 防御
    Defense = 2,
    /// 最大生命
    MaxHealth = 3,
    /// 生命
    Health = 4,
    ///最大数量
    MaxCount,
}
impl From<i32> for EAttributeType{    
    fn from(attr: i32) -> Self {        
        use EAttributeType::*;
        match attr{
            1 => Attack,
            2 => Defense,
            3 => MaxHealth,
            4 => Health,
            _ => None,
        }
    }
}
impl Into<i32> for EAttributeType{
    fn into(self) -> i32 {
        self as i32
    }
}
#[allow(unused)]
impl EAttributeType{
    #[inline]
    ///获取所有属性迭代器,有alloc
    pub fn iter()-> std::vec::IntoIter<EAttributeType>{
        (1..EAttributeType::MaxCount as i32).into_iter().map(|i| i.into()).collect::<Vec<_>>().into_iter()
    }
}
#[repr(C)]
///属性绑定器
#[derive(Debug, Clone, Default)]
pub struct AttributeBinder{
    raw: [i32 ;EAttributeType::MaxCount as usize]
}
impl AttributeBinder{
    #[allow(unused)]
    pub fn new() -> Self{
        Self::default()
    }
    #[inline]
    pub fn get_attr(&self, attr: EAttributeType) -> i32{
        self.raw[attr as usize] 
    }
    #[inline]
    ///get attribute with sub attributer
    pub fn get_attr_with(&self, attr: EAttributeType, other: &Option<Self>) -> i32{
        self.get_attr(attr) + other.as_ref().map(|other| other.get_attr(attr)).unwrap_or_default()
    }
    #[inline]
    pub fn set_attr(&mut self, attr: EAttributeType, v: i32){
        self.raw[attr as usize] = v;
    }
    #[inline]
    pub fn update_attr(&mut self, attr: EAttributeType, v: i32){
        self.raw[attr as usize] += v;
    }
    #[inline]
    pub fn clear(&mut self){
        self.raw.fill(0);
    }
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<i32>{
        self.raw.iter()
    }
}
impl From<BTreeMap<i32, i32>> for AttributeBinder{
    fn from(map: BTreeMap<i32, i32>) -> Self {
        let mut binder= Self::default();
        map.into_iter().for_each(|(k,v)| binder.set_attr(k.into(), v));
        binder
    }
}