use anyhow::Result;
use moka::future::Cache;
use sqlx::SqlitePool;
use url::Url;

use crate::objects::users::ProtocolUser;

#[derive(Debug, Clone)]
pub struct AmallgamContext {
    server_base_url: Url,

    pub(crate) remote_user_cache: Cache<Url, ProtocolUser>,

    db_connection: SqlitePool,
}

impl AmallgamContext {
    pub async fn new(server_base_url: Url, db_url: &str) -> Result<Self> {
        let ctx = AmallgamContext {
            server_base_url,

            remote_user_cache: Cache::new(1024),

            db_connection: SqlitePool::connect(db_url).await?,
        };

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        // TODO: Remove this temporary user
        let user = ctx.new_bot_user("def");
        ctx.upsert_user(user).await.unwrap();

        Ok(ctx)
    }

    pub(crate) fn server_relative_url(&self, suffix: &str) -> Url {
        self.server_base_url
            .join(suffix)
            .expect("Bad Server-relative URL?")
    }

    pub(crate) fn db_connection(&self) -> &SqlitePool {
        &self.db_connection
    }
}
