use anyhow::Result;
use moka::future::Cache;
use sqlx::SqlitePool;
use url::Url;

use crate::objects::{notes::Note, users::ProtocolUser};

#[derive(Debug, Clone)]
pub struct AmallgamContext {
    server_base_url: Url,

    remote_user_cache: Cache<Url, ProtocolUser>,
    note_cache: Cache<Url, Note>,

    db_connection: SqlitePool,
}

impl AmallgamContext {
    pub async fn new(server_base_url: Url, db_url: &str) -> Result<Self> {
        let ctx = AmallgamContext {
            server_base_url,

            remote_user_cache: Cache::new(1024),
            note_cache: Cache::new(1024),

            db_connection: SqlitePool::connect(db_url).await?,
        };

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        // TODO: Remove this temporary user
        let user = ctx.new_bot_user("abc");
        ctx.upsert_user(&user).await.unwrap();

        Ok(ctx)
    }

    pub(crate) fn server_base_url(&self) -> &Url {
        &self.server_base_url
    }

    pub(crate) fn is_local_url(&self, url: &Url) -> bool {
        url.scheme() == self.server_base_url.scheme() && url.host() == self.server_base_url.host()
    }

    pub(crate) fn db_connection(&self) -> &SqlitePool {
        &self.db_connection
    }
}
