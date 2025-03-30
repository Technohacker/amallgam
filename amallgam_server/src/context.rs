use activitypub_federation::http_signatures::generate_actor_keypair;
use anyhow::Result;
use sqlx::SqlitePool;

use crate::objects::users::User;

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

        // TODO: Remove this temporary user
        let kp = generate_actor_keypair().unwrap();
        ctx.upsert_user(User {
            id: "https://amallgam.docker/user/abc".parse().unwrap(),
            preferred_username: "abc".into(),
            name: "Abc".into(),
            inbox: "https://amallgam.docker/user/abc/inbox".parse().unwrap(),
            outbox: "https://amallgam.docker/user/abc/outbox".parse().unwrap(),
            public_key_pem: kp.public_key,
            private_key_pem: Some(kp.private_key),
            shared_inbox: None,
        })
        .await
        .unwrap();

        Ok(ctx)
    }

    pub(crate) fn db_connection(&self) -> &SqlitePool {
        &self.db_connection
    }
}
