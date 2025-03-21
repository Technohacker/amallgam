use anyhow::Result;
use sqlx::SqlitePool;
use url::Url;

use crate::{query_builders, users::User};

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

    pub async fn upsert_user(&self, user: User) -> Result<()> {
        sqlx::query(
            "
            INSERT INTO users (
                id,
                preferred_username,
                name,
                inbox,
                outbox,
                public_key
            ) VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                json($6)
            ) ON CONFLICT UPDATE SET
                preferred_username = $2,
                name = $3,
                inbox = $4,
                outbox = $5,
                public_key = $6
            ",
        )
        .bind(user.id.to_string())
        .bind(&user.preferred_username)
        .bind(&user.name)
        .bind(user.inbox.to_string())
        .bind(user.outbox.to_string())
        .bind(serde_json::to_string(&user.public_key)?)
        .execute(&self.db_connection)
        .await?;

        Ok(())
    }

    pub async fn get_user_by_id(&self, url: &Url) -> Result<Option<User>> {
        let row = query_builders::select_user()
            .push("WHERE id = $1")
            .build_query_as()
            .bind(url.as_str())
            .fetch_optional(&self.db_connection)
            .await?;

        Ok(row)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let row = query_builders::select_user()
            .push("WHERE preferred_username = $1")
            .build_query_as()
            .bind(username)
            .fetch_optional(&self.db_connection)
            .await?;

        Ok(row)
    }
}
