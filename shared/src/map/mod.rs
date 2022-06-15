//! 地图相关逻辑
pub mod pathfinding;
mod map;
use core::fmt::Debug;
use std::ops::{Add, AddAssign, Mul, Sub};

pub use map::*;

static DIRS: [Position; 4] = [
    Position::new(0, 0),
    Position::new(1, 0),
    Position::new(0, 1),
    Position::new(1, 1)
];
pub fn base_dir_index(dir: &Position) -> usize{ 
    for index in 0..4{
        if &DIRS[index] == dir{
            return index;
        }
    }
    return usize::MAX;
}
pub fn inverse_direction(dir: Position) -> Position{
    let index = DIRS.iter().enumerate().find(|(_,p)| **p == dir).map(|t| t.0).unwrap_or(0);
    DIRS[index]
}
pub fn is_base_dir<'a, T>(dir: &'a T) -> bool where Position: From<&'a T>{
    let p: Position = Position::from(dir);
    for d in &DIRS {
        if &p == d { return true;}
    }
    return false;
}
#[repr(C)]
///地图坐标
#[derive(Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Position{
    pub x: i32,
    pub y: i32,
}
impl Position{
    pub const fn new(x: i32, y: i32) -> Self{
        Self{x,y}
    }
    #[inline]
    pub fn magnitude(&self)->u32{         
        (self.x.pow(2) + self.y.pow(2)) as u32
    }
    #[inline]
    pub fn sqr_magnitude(&self) -> f64{         
        (self.magnitude() as f64).sqrt()
    }
    //格子距离
    pub fn distance<'a, T: 'static>(&self, other: &'a T) -> u32 where Self: From<&'a T>{
        let other: Self = Self::from(other);
        [(self.x-other.x).abs(),
        ((-self.y -self.x)- (-other.y - other.x)).abs(),
        (self.y - other.y).abs()]
        .iter()
        .fold(0, |max,cur| max.max(*cur)) as u32
    }
    ///归一化
    pub fn normalize(&mut self) -> Position{
        if self.x == 0 && self.y == 0{ return *self; }
        if self.x == 0{
            self.y /= self.y.abs();
            return *self;
        }
        else if self.y == 0{
            self.x /= self.x.abs();
            return *self;
        }
        let gcd = get_gcd(self.x, self.y).abs();
        self.x /= gcd; self.y /= gcd;
        *self
    }
}
pub fn get_gcd<T: std::ops::Rem<Output = T> + Default + PartialEq + Copy>(a: T, b: T) -> T{
    if a % b == T::default(){ return b;}
    return get_gcd(b, a % b);
}
impl From<Point2> for Position{
    fn from(p: Point2) -> Self {
        Self{ x: p.x, y: p.y}
    }
}
impl From<&Point2> for Position{
    fn from(p: &Point2) -> Self {
        Self{ x: p.x, y: p.y}
    }
}
impl From<&Position> for Position{
    fn from(p: &Position) -> Self {
        *p
    }
}
///地图坐标
#[derive(Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Point2{
    pub x: i32,
    pub y: i32,
    ///第15位: 阻挡
    /// 0-14位: 序号
    state: u16,
}
impl Point2{
    pub const fn new(x: i32, y: i32) -> Self{
        Self{x,y, state: 0}
    }
    pub const fn with_pos(x: i32, y: i32, state: u16) -> Self{
        Self{x, y, state}
    }
    pub fn generate_state(barrier: bool, id: u16) -> u16{
        crate::if_else!(barrier,1,0) << 15 | id
    }
    #[inline]
    pub fn magnitude(&self)->u32{         
        (self.x.pow(2) + self.y.pow(2)) as u32
    }
    #[inline]
    pub fn sqr_magnitude(&self) -> f64{         
        (self.magnitude() as f64).sqrt()
    }
    #[inline]
    pub fn set_state(&mut self, state: u16){
        self.state= state;
    }
    #[inline]
    pub fn get_state(&self)-> u16 { self.state}
    #[inline]
    pub fn barrier(&self) -> bool { (self.state & 0x8000) != 0}
    #[inline]
    pub fn id(&self) -> u16 { self.state & 0x7FFF }
    ///六边形格子距离
    #[inline]
    pub fn distance(&self, other: &Self) -> u32{
        [(self.x-other.x).abs(),
        ((-self.y -self.x)- (-other.y - other.x)).abs(),
        (self.y - other.y).abs()]
        .iter()
        .fold(0, |max,cur| max.max(*cur)) as u32
    }
    ///归一化
    pub fn normalize(&mut self) -> Point2{
        if self.x == 0 && self.y == 0{ return *self; }
        if self.x == 0{
            self.y /= self.y.abs();
            return *self;
        }
        else if self.y == 0{
            self.x /= self.x.abs();
            return *self;
        }
        let gcd = get_gcd(self.x, self.y).abs();
        self.x /= gcd; self.y /= gcd;
        *self
    }
}
impl PartialEq for Point2{
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}
impl Eq for Point2{}
impl Debug for Point2{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point2")
        .field("x", &self.x)
        .field("y", &self.y)
        .field("id", &self.id()).finish()
    }
}
impl PartialOrd for Point2{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.magnitude().partial_cmp(&other.magnitude())
    }
}
impl Ord for Point2{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.magnitude().cmp(&other.magnitude())
    }
}
impl Mul<usize> for Point2{
    type Output = Point2;

    fn mul(self, rhs: usize) -> Self::Output {
        Self{x: self.x * (rhs as i32), y: self.y * (rhs as i32), state: self.state}
    }
}
impl Mul<i32> for Point2{
    type Output = Point2;

    fn mul(self, rhs: i32) -> Self::Output {
        Self{x: self.x * rhs, y: self.y * rhs, state: self.state}
    }
}
impl Add for Point2{
    type Output = Point2;

    fn add(self, rhs: Self) -> Self::Output {
        Self{x: self.x + rhs.x, y: self.y + rhs.y, state: self.state}
    }
}
impl AddAssign for Point2{
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl Sub for Point2{
    type Output = Point2;

    fn sub(self, rhs: Self) -> Self::Output {
        Self{x: self.x - rhs.x, y: self.y - rhs.y, state: self.state}
    }
}
impl PartialEq<Position> for Point2{
    fn eq(&self, other: &Position) -> bool {
        self.x == other.x && self.y == other.y
    }
}
impl PartialEq<Point2> for Position{
    fn eq(&self, other: &Point2) -> bool {
        self.x == other.x && self.y == other.y
    }
}
impl PartialEq for Position{
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for Position{}
impl Debug for Position{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Position")
        .field("x", &self.x)
        .field("y", &self.y).finish()
    }
}
impl PartialOrd for Position{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.magnitude().partial_cmp(&other.magnitude())
    }
}
impl Ord for Position{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.magnitude().cmp(&other.magnitude())
    }
}
impl Mul<usize> for Position{
    type Output = Position;

    fn mul(self, rhs: usize) -> Self::Output {
        Self{x: self.x * (rhs as i32), y: self.y * (rhs as i32)}
    }
}
impl Mul<i32> for Position{
    type Output = Position;

    fn mul(self, rhs: i32) -> Self::Output {
        Self{x: self.x * rhs, y: self.y * rhs}
    }
}
impl Add for Position{
    type Output = Position;

    fn add(self, rhs: Self) -> Self::Output {
        Self{x: self.x + rhs.x, y: self.y + rhs.y}
    }
}
impl AddAssign for Position{
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl Sub for Position{
    type Output = Position;

    fn sub(self, rhs: Self) -> Self::Output {
        Self{x: self.x - rhs.x, y: self.y - rhs.y}
    }
}
impl Into<Point2> for Position{
    fn into(self) -> Point2 {
        Point2::new(self.x, self.y)
    }
}
impl Into<Point2> for &Position{
    fn into(self) -> Point2 {
        Point2::new(self.x, self.y)
    }
}