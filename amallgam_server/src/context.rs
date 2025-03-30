use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AmallgamContext {
    db_connection: SqlitePool,
}

impl AmallgamContext {
    pub async fn new(db_url: &str) -> Result<Self> {
        let ctx = AmallgamContext {
            db_connection: SqlitePool::connect(db_url).await?,
        };

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        Ok(ctx)
    }

    pub(crate) fn db_connection(&self) -> &SqlitePool {
        &self.db_connection
    }
}
