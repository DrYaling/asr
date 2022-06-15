//! ini配置文件解析工具
//! 
//!     config.ini = 
//! 
//!     [配置文件]
//!     #alpha config
//!     alpha = 1
//!     #beta config
//!     beta = 173
//! 
//!     [服务地址配置]
//!     #remote host
//!     host = 127.0.0.1:8080
//!     #local binding
//!     binding = 0.0.0.0:8082
//!
//! 
//! 
//!     println!("ini config info \r\n{:?}",ini::parse("config.ini"));
//! 
//!     Ok({"alpha": "1", "beta": "173", "binding": "0.0.0.0:8082", "host": "127.0.0.1:8080"})
use std::collections::BTreeMap;
use std::io::{BufReader, BufRead};
pub fn parse(file_name: &str) -> Result<BTreeMap<String,String>,String>{
    let mut map = BTreeMap::new();
    match std::fs::File::open(file_name){
        Ok(content) => {
            let mut lines_iter = BufReader::new(content).lines().map(|l| l.unwrap());
            loop {
                match lines_iter.next(){
                    Some(line) => {
                        //默认2字节以下不处理
                        if line.len() > 2{
                            let lstr = line.trim_start().trim_end().as_bytes();
                            //[title]匹配
                            if lstr[0usize] as char == '['{
                                if lstr[lstr.len() -1] as char != ']'{
                                    //title不匹配
                                    return Err(text!("配置title错误 {}",line));
                                }
                                //忽略title继续判断下一行
                                continue;
                            }
                            //#注释筛选
                            if lstr[0usize] as char == '#'{
                                continue;
                            }
                            //A=B配置内容提取
                            if line.contains("="){
                                let splites: Vec<&str> = line.split("=").collect();
                                if splites.len() < 2{
                                    return Err(text!("配置错误 {}",line));
                                }
                                //key有效性判断
                                let key = splites[0usize].trim_start().trim_end();
                                let none_alpha_num: Vec<&str> = key.matches(|c: char| !c.is_ascii_alphabetic() && !c.is_numeric() && c != '_').collect();
                                if none_alpha_num.len() > 0{
                                    return Err(text!("配置错误 {},不允许特殊字符(包括回车换行)",line));
                                }
                                else {
                                    let value = if splites.len() == 2{
                                        splites[1usize].trim_start().trim_end().trim_end_matches(|c| c == '\"').trim_start_matches(|c| c == '\"').to_string()
                                    }else{
                                        let mut ret = splites[1usize].trim_start().trim_end().trim_end_matches(|c| c == '\"').trim_start_matches(|c| c == '\"').to_string();
                                        for i in 2..splites.len(){
                                            ret += splites[i].trim_start().trim_end().trim_end_matches(|c| c == '\"').trim_start_matches(|c| c == '\"');
                                        }
                                        ret
                                    };
                                    // if value.contains(",") || value.contains("%") || value.contains("&") || value.contains(">") || value.contains("<") || value.contains("!"){
                                    //     return Err(text!("配置错误 {},不允许特殊字符(, % & < > !)",line));
                                    // }
                                    map.insert(key.to_string(),value);
                                    continue;
                                }
                            }
                            return Err(text!("配置错误 {},无法识别的配置行",line));
                        }
                    },
                    None => break,
                }
            }
        },
        Err(e) => {
            log_error!("fail to open file {}",file_name);
            return Err(e.to_string());
        }
    }
    Ok(map)
}