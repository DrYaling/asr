//! 六边形地图数据
//! 
use std::slice::Iter;

use crate::if_else;

use super::Point2;
const MAX_MAP_WIDTH: usize = 400;
///最大地图id
pub const MAX_MAP_ID: usize = MAX_MAP_WIDTH * MAX_MAP_WIDTH + MAX_MAP_WIDTH + MAX_MAP_WIDTH;
const DIRS: [Point2; 4] = [
    Point2::new(0, 0),
    Point2::new(1, 0),
    Point2::new(0, 1),
    Point2::new(1, 1)
];
///获取一个点周围的几个点
pub fn ring(center: Point2, radius: usize) -> Vec<Point2>{
    let mut r = Vec::new();
    let mut point = center + DIRS[4] * radius;
    for i in 0..4 {
        for _j in 0..radius {
            r.push(point);
            point += DIRS[i]
        }
    }
    r
}
pub struct MapBuilder{
    barriers: Vec<u16>,
    width: u16, 
    height: u16,
    horizontal_map: bool,
    map_id: u32,
}
impl MapBuilder{
    pub fn with_barriers<T: IntoIterator<Item = u16>>(&mut self, barriers: T) -> &mut Self{
        self.barriers = barriers.into_iter().collect();
        self
    }
    pub fn new (map_id: u32, width: u16, height: u16, horizontal_map: bool)-> Self{
        Self{width, height, barriers: Default::default(), horizontal_map, map_id}
    }
    pub fn build(&mut self)-> Map{
        let mut rp = Vec::new();
        std::mem::swap(&mut rp, &mut self.barriers);
        let mut map = Map::new(self.map_id, self.width, self.height,rp, self.horizontal_map);
        map.init();
        map
    }
}
///```
/// //地图数据
/// let map = shared::map::MapBuilder::new(10,10).with_barriers(vec![10,20,32]).build();
/// ```
#[derive(Debug, Default)]
pub struct Map{
    map_id: u32,
    ///true-横向地图, false-纵向地图
    horizontal_map: bool,
    width: u16,
    height: u16,
    ///阻挡格子
    barriers: Vec<u16>,
    ///格子
    grid: Vec<Point2>,   
}
impl Map{
    fn new(map_id: u32, width: u16, height: u16, barriers: Vec<u16>, horizontal_map: bool) -> Self{
        Self{
            map_id, width, height, barriers, grid: Self::create_map(width,height, horizontal_map), horizontal_map,
        }
    }
    #[allow(unused)]
    #[inline]
    pub(crate) fn grid(&self) -> &Vec<Point2> { &self.grid }
    #[inline]
    pub fn map_id(&self) -> u32 { self.map_id }
    #[inline]
    pub fn height(&self) ->u16 { self.height }
    #[inline]
    pub fn width(&self) ->u16 { self.width }
    fn create_map(width: u16, height: u16, horizontal_map: bool) -> Vec<Point2>{
        fn block(center: Point2, radius: usize) -> Vec<Point2>{
            let mut r = Vec::new();
            r.push(center);
            for i in 1..=radius {
                r.append(&mut ring(center,i));
            }
            r
        }
        let check_boundary = move |x: f32, y: f32| -> bool{ 
            let (height, width) = (height as f32, width as f32);
            if horizontal_map{
                if_else!( y < 0f32 || y > height - 1.0 || x > width + 1.0 - (y / 2.0 + 0.5) || x < -(y / 2.0 + 0.5), 
                false, true)
            }
            else{
                if_else!(
                    x < 0.0 || y < -( x / 2.0 + 0.5) || x > width - 1.0 || y > (height - 1.0 - x / 2.0) + 0.5,
                    false, true
                )
            }
        };
        block(Point2::default(), MAX_MAP_WIDTH)
        .into_iter()
        .filter(|point|{
            let q = point.x as f32;
            let r = point.y as f32;
            check_boundary(q, r)
        })
        .collect()
    }
    ///初始化地图,刷新格子状态
    pub fn init(&mut self){
        let barriers = self.barriers.clone();
        let point_id = move |target: Point2| -> u16{
            (target.x * target.y).max(0) as u16
        };
        self.grid.iter_mut().for_each(|point|{
            let id = point_id(point.clone());
            point.set_state(Point2::generate_state(barriers.iter().find(|b| **b == id).is_some(), id));
        });
    }
    #[inline]
    pub fn iter(&self) -> Iter<Point2>{
        self.grid.iter()
    }
    #[inline]
    pub fn barrier_iter(&self) -> Iter<u16>{
        self.barriers.iter()
    }
    ///获取无阻挡格子
    #[inline]
    pub fn get_free_by_id(&self, id: u16) -> Option<Point2>{
        if id as usize > self.grid.len(){
            return None;
        }
        let point = self.grid.iter().find(|p| p.id() == id).cloned()?;
        if self.barriers.contains(&point.id()){
            return None;
        }
        Some(point)
    }
    #[inline]
    pub fn get_point_by_id(&self, id: u16) -> Option<Point2>{
        if id as usize > self.grid.len(){
            return None;
        }
        self.grid.iter().find(|p| p.id() == id).cloned()
    }
    #[allow(unused)]
    #[inline]
    fn check_boundary(&self, target: Point2) -> bool{ 
        target.x >= 0 && target.x <= self.width as i32 && target.y >= 0 && target.y <= self.height as i32
    }
    ///获取点位id
    #[inline]
    fn point_id(&self, target: Point2) -> u16{
        (target.x * target.y).max(0) as u16
    }
    ///改变格子的状态
    pub fn change_state(&mut self, grid: u16, block: bool){
        match block {
            true => {
                //如果有这个位置,加进阻挡列表
                if self.grid.iter().find(|p| p.id() == grid).is_some(){
                    self.barriers.push(grid);
                }
            },
            false => {
                if let Some(index) = self.barriers.iter().enumerate().find(|(_,id)| **id == grid).map(|t| t.0){
                    self.barriers.remove(index);
                }
            },
        }
    }
    ///获取点位id
    #[inline]
    pub fn get_point_id(&self, target: Point2) -> u16{ 
        self.point_id(target)
    }
    #[inline]
    pub fn set_point_state(&self, mut target: Point2) -> Point2{ 
        let id = self.get_point_id(target);
        let state = Point2::generate_state(self.barriers.iter().find(|b| **b == id).is_some(), id);
        target.set_state(state);
        target
    }
    ///将点绑定到地图上,设置阻挡和序号等
    #[inline]
    pub fn bind_point(&self, point: &mut Point2){
        let id = self.get_point_id(*point);
        let state = Point2::generate_state(self.barriers.iter().find(|b| **b == id).is_some(), id);
        point.set_state(state);
    }
    ///将2个点想加,并绑定点位
    pub fn add_point(&self, p0: Point2, p1: Point2) -> Point2{ 
        let mut pr = p0 + p1;
        self.bind_point(&mut pr);
        return pr;
    }
    // ///获取某点的可进行区域
    // fn get_successors(&self, point: &Point2) -> impl IntoIterator<Item=(Point2,u32)>{
    //     DIRS.iter().map(|p| (*point + *p, 1u32)).filter(|t| {
    //         !self.barriers.contains(&self.get_point_id(t.0))
    //     }).collect::<Vec<_>>()
    // }
    ///寻路
    pub fn get_path<T: Fn(&Point2) -> bool, P: Fn(&Point2) -> bool>(&self, start: Point2, end: Point2, filter: &T, predicter: &P) -> Option<Vec<Point2>>{
        let path = MapPath{map: self, target: end, filter};
        let result = super::pathfinding::a_star_search(start, predicter, &path);
        if result.success{
            result.steps.into()
        }
        else{
            None
        }
    }
}
///private path finding binder for Map
struct MapPath<'a>{
    map: &'a Map,
    target: Point2,
    filter: &'a dyn Fn(&Point2) -> bool
}
impl<'a> super::pathfinding::BaseMap for MapPath<'a>{
    fn get_available_exits(&self, point: Point2) -> smallvec::SmallVec<[Point2; 6]> {
        //let mut sorted = smallvec::SmallVec::<[Point2; 6]>::new();
        let mut unsorted = smallvec::SmallVec::<[Point2; 6]>::new();
        for p in DIRS {
            let mut pos = p + point;
            self.map.bind_point(&mut pos);
            if self.map.barriers.contains(&pos.id()) || !(self.filter)(&pos){
                continue;
            }
            unsorted.push(pos);
        }
        // while sorted.len() != unsorted.len(){
        //     let mut min_dis = u32::MAX;
        //     let mut p = unsorted.first().copied().unwrap();
        //     //离目标最近的点先计算
        //     for pos in &unsorted{
        //         let dis = self.distance_to_end(pos);
        //         if dis < min_dis{
        //             p = *pos;
        //             min_dis = dis;
        //         }
        //     }
        //     sorted.push(p);
        // }
        unsorted
    }

    fn distance_to_end(&self, point: &Point2) -> u32 {
        point.distance(&self.target)
    }

    fn success_check(&self, point: &Point2) -> bool{ 
        point == &self.target
     }
}
#[cfg(test)]
#[test]
fn test_map(){
    let map = crate::map::MapBuilder::new(1, 10,10, true).with_barriers(vec![10,20,32]).build(); 
    let target = Point2::new(1,3);
    let path = map.get_path(Point2::new(0, 1), target, &|_| true, &|p| p == &target);
    println!("path {:?}",path);
    println!("dis from (0,0) to (0,2) {}", Point2::new(0, 0).distance(&Point2::new(0, 2)));
}
