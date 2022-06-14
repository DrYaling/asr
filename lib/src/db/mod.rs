mod db_pool;
use std::{fmt::{Display}};

pub use db_pool::{start_pools, send_query, get_pool, DbPoolInfo, PooledDbInfo};
#[derive(Debug)]
pub struct RpcCommand<T>{
    pub rpc_squence: u32,
    pub data: T,
}
impl<T> RpcCommand<T>{
    pub fn new(rpc: u32, t: T) -> Self{
        Self{ rpc_squence: rpc, data: t}
    }
}
impl<T> Clone for RpcCommand<T> where T: Clone{
    fn clone(&self) -> Self {
        Self { rpc_squence: self.rpc_squence.clone(), data: self.data.clone() }
    }
}
#[derive(Debug, Clone)]
pub struct DbQueryError{
    rpc: u32,
    pub code: i32,
    pub msg: String,
}
impl DbQueryError{
    pub fn rpc(&self) -> u32 { self.rpc }
}

impl Display for DbQueryError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("db err: ({},{},r: {})",self.code,self.msg,self.rpc))
    }
}
impl std::error::Error for DbQueryError {}
impl From<sqlx::Error> for DbQueryError{
    fn from(err: sqlx::Error) -> Self {
        let db_err= err.as_database_error();
        if let Some(err) = db_err{
            Self{
                rpc: 0,
                code: 2,
                msg: err.to_string()
            }
        }
        else{
            Self{
                rpc: 0,
                code: 1,
                msg: err.to_string()
            }
        }
    }
}
#[derive(Debug)]
pub enum DbCommand<T>{
    ///rpc查询
    RpcQuery(RpcCommand<Option<T>>),
    ///正常查询
    Normal(Option<T>),
    Err(DbQueryError)
}
impl<T> DbCommand<T>{
    #[inline]
    pub fn rpc(rpc: u32, t: T) -> Self{
        Self::RpcQuery(RpcCommand::new(rpc, t.into()))
    }
    #[inline]
    pub fn normal(t: T) -> Self{
        Self::Normal(t.into())
    }
    #[inline]
    ///构造一个带rpc信息的默认命令
    pub fn rpc_default(rpc: u32) -> Self{
        match rpc{ 
            0 => Self::normal_default(),
            r => Self::RpcQuery(RpcCommand::new(r, None))
        }
        
    }
    #[inline]
    ///构造一个不带rpc信息的默认命令
    pub fn normal_default() -> Self{
        Self::Normal(None)
    }
    ///将命令展开为(rpc,data)的元组
    pub fn flat(self) -> Result<(u32,T),u32>{
        match self{
            DbCommand::RpcQuery(cmd) if cmd.data.is_some()=> Ok((cmd.rpc_squence,cmd.data.unwrap())),
            DbCommand::Normal(t) if t.is_some() => Ok((0,t.unwrap())),
            DbCommand::RpcQuery(cmd)  => {
                Err(cmd.rpc_squence)
            }
            _ => {
                Err(0)
            }
        }
    }
    pub fn ref_flat(&self) -> Result<(u32,&T),u32>{
        match self{
            DbCommand::RpcQuery(cmd) if cmd.data.is_some()=> Ok((cmd.rpc_squence,cmd.data.as_ref().unwrap())),
            DbCommand::Normal(t) if t.is_some() => Ok((0,t.as_ref().unwrap())),
            DbCommand::RpcQuery(cmd)  => {
                Err(cmd.rpc_squence)
            }
            _ => {
                Err(0)
            }
        }
    }
    #[inline]
    ///如果不是Err,设置值
    pub fn set(&mut self, v: T){
        match self{
            DbCommand::RpcQuery(rpc) => rpc.data = v.into(),
            DbCommand::Normal(t) => (*t)= v.into(),
            _ => (),
        }
    }
    #[inline]
    ///如果不是Err,设置值
    pub fn set_opt(&mut self, v: Option<T>){
        match self{
            DbCommand::RpcQuery(rpc) => rpc.data = v,
            DbCommand::Normal(t) => (*t)= v,
            _ => (),
        }
    }
    #[inline]
    pub fn err(code: i32, msg: String) -> Self{
        DbCommand::Err(DbQueryError{code,msg, rpc: 0})
    }
    #[inline]
    pub fn to_err(&mut self, msg: String){
        match self{
            DbCommand::RpcQuery(rpc) => *self= DbCommand::Err(DbQueryError{code: -1,msg, rpc: rpc.rpc_squence}),
            DbCommand::Normal(_) => *self= Self::err(0,msg),
            _ => (),
        }
    }
    pub fn cloned(&self) -> Self where T: Clone{
        self.clone()
    }
}
impl<T> Clone for DbCommand<T> where T: Clone{
    fn clone(&self) -> Self {
        match self {
            Self::RpcQuery(arg0) => Self::RpcQuery(arg0.clone()),
            Self::Normal(arg0) => Self::Normal(arg0.clone()),
            Self::Err(arg0) => Self::Err(arg0.clone()),
        }
    }
}
pub type DbResult<T> = Result<DbCommand<T>,DbCommand<T>>;
pub trait MergeDbResult<B,T> : Sized{
    fn merge(self, _: DbCommand<T>) -> Self{ self }
    fn merge_to(self, e: &DbCommand<T>) -> Result<B,DbCommand<T>>;
}
impl<T,B,R> MergeDbResult<T,B> for Result<T,R> where R: ToString{
    fn merge_to(self, e: &DbCommand<B>) -> Result<T,DbCommand<B>> {
        match self{
            Err(r) => {
                let e = match e{
                    DbCommand::RpcQuery(arg) => DbCommand::Err(DbQueryError{rpc: arg.rpc_squence, code: -1, msg: "MergeDbResult from ".to_string() + &r.to_string()}),
                    DbCommand::Normal(_) => DbCommand::err(-1, "MergeDbResult from ".to_string() + &r.to_string()),
                    DbCommand::Err(e) => DbCommand::Err(e.clone()),
                };
                Err(e)
            }
            Ok(t) => {
                Ok(t)  
            },
        }
    }
}
///解包DbResult
#[inline]
pub fn unwrap_cmd<T>(cmd: DbResult<T>) -> DbCommand<T>{
    match cmd{
        Ok(cmd) => cmd,
        Err(cmd) => cmd,
    }
}