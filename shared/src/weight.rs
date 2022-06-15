//! 权重计算

use rand::Rng;

pub trait WeightCalculater<T: Clone>{
    ///计算权重,并返回对象的copy
    fn weight_cloned(&self, weights: &Vec<i32>) -> Option<T>;
}
impl<T: Clone> WeightCalculater<T> for &Vec<T>{
    fn weight_cloned(&self, weights: &Vec<i32>) -> Option<T> {
        (*self).weight_cloned(weights)
    }
}
impl<T: Clone> WeightCalculater<T> for Vec<T>{
    fn weight_cloned(&self, weights: &Vec<i32>) -> Option<T> {
        if weights.len() != self.len(){
            log_error!("计算权重失败,权重列表 {} 和对象列表 {} 长度不一致", weights.len(), self.len());
            return None;
        }
        //计算权重
        let total_weight = weights.iter().fold(0, |r, x| r + *x);
        let mut weight: i32 = if_else!(total_weight <= 1, 1, rand::thread_rng().gen_range(0..=total_weight));
        let index = weights.iter().enumerate().find(|(_, x)| {
            let w = **x;
            if weight <= w{
                true
            }
            else{
                weight -= w;
                false

            }
        }).map(|t| t.0)?;
        self[index].clone().into()
    }
}