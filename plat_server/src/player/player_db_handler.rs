use crossbeam::channel::*;
use futures::FutureExt;
use lib::{db::{DbCommand, DbResult, MergeDbResult}};
use crate::{player::*};
#[derive(Default, Debug)]
pub struct DbHandler{
    ///玩家登录消息
    player_info_handler: Option<Receiver<DbResult<PlayerLoginInfo>>>
}
impl DbHandler{
    ///查询玩家登录信息
    pub fn load_player_info(&mut self, account: &str, access_token: &str, cmd: DbCommand<PlayerLoginInfo>)-> anyhow::Result<()>{
        let (tx,rx) = bounded(1);
        self.player_info_handler = rx.into();
        let account = account.to_string();
        let access_token = access_token.to_string();
        lib::db::send_query(Box::new(async move {
            info!("load player info {}",account);
            tx.send(DbHandler::on_load_player(&account,&access_token,cmd).await)
            .map_err(|_| error!("fail to send player info {}",account)).ok();
        }).boxed())?;
        Ok(())
    }
    ///加载角色列表
    pub async fn load_characters(player_id: u64) -> anyhow::Result<Vec<CharacterLoader>> {
        let pool = lib::db::get_pool("bg_db_server")?;
        let characters = sqlx::query_as::<_,CharacterLoader>("SELECT id,role_id,own_type,state FROM db_player_character WHERE player_id=?")
        .bind(player_id)
        .fetch_all(pool.as_ref()).await.map_err(|e| logthrow!(e,e))?;
        Ok(characters)
    }
    ///load player, if not exist, create player
    async fn on_load_player(account: &str, access_token: &str,mut cmd: DbCommand<PlayerLoginInfo>) -> DbResult<PlayerLoginInfo>{
        let pool = lib::db::get_pool("bg_db_server").merge_to(&cmd)?;
        let result = sqlx::query_as::<_,(u64,String,String)>("SELECT id,name,access_token FROM db_player WHERE account=?")
        .bind(&account)
        .fetch_optional(pool.as_ref()).await.merge_to(&cmd).map_err(|e| logthrow!(e,e))?;
        if let Some((player_id,name,token)) = result{
            if access_token != &token{
                warn!("player {} token invalid expected {}, got {}",account,token, access_token);
                cmd.to_err("token校验失败".to_string());
                return Err(cmd);
            }
            let mut characters = sqlx::query_as::<_,CharacterLoader>("SELECT id,role_id,own_type,state FROM db_player_character WHERE player_id=?")
            .bind(player_id)
            .fetch_all(pool.as_ref()).await.merge_to(&cmd).map_err(|e| logthrow!(e,e))?;
            if characters.len() == 0{
                let default_characters = if let Some(r) = lib::libconfig::common::get_str("InitialRole"){
                    let vs = r.split("|").collect::<Vec<_>>();
                    vs.iter().filter_map(|s| s.parse::<u32>().ok()).collect::<Vec<_>>()
                }
                else{
                    vec![10111,10211,10311,10411]
                };
                for cid in default_characters {
                    let query = sqlx::query("INSERT INTO db_player_character (role_id,player_id,own_type,state) VALUES(?,?,0,1)")
                    .bind(cid).bind(player_id)
                    .execute(pool.as_ref()).await.merge_to(&cmd).map_err(|e| logthrow!(e,e))?;
                    characters.push(CharacterLoader{
                        id: query.last_insert_id(),
                        role_id: cid,
                        own_type: 0,
                        state: 1
                    });
                }
            }
            info!("player [{}] load success, name {}, characters :{:?}",account,name,characters);
            cmd.set(PlayerLoginInfo::new(player_id, name, characters));
            Ok(cmd)
        }
        else{
            warn!("player {} load fail (account not exist)", account);
            cmd.to_err("没有这个玩家".to_string());
            Err(cmd)
        }
    }
    ///尝试获取玩家的查询结果,如果结果还未查到,返回Err(())
    pub fn try_get_player_info(&self) -> Result<DbResult<PlayerLoginInfo>,()>{
        match &self.player_info_handler{
            Some(handler) => handler.try_recv().map_err(|_| ()),
            None => Err(())
        }
    }
}