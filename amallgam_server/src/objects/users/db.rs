use activitypub_federation::fetch::object_id::ObjectId;
use sqlx::{sqlite::SqliteRow, Database, FromRow, QueryBuilder, Row};
use url::Url;

use crate::context::AmallgamContext;

use super::User;

impl AmallgamContext {
    fn select_user<'args, DB: Database>() -> QueryBuilder<'args, DB> {
        QueryBuilder::new(
            "
                SELECT
                    users.fed_id as fed_id,
                    users.preferred_username,
                    users.name,
                    users.inbox,
                    users.outbox,
                    users.public_key,
                    users.shared_inbox,
                    bot_users.private_key
                FROM users
                LEFT JOIN bot_users
                    ON users.fed_id = bot_users.fed_id
                ",
        )
    }

    pub async fn upsert_user(&self, user: &User) -> anyhow::Result<()> {
        sqlx::query(
            "
            INSERT INTO users (
                fed_id,
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
                $6
            ) ON CONFLICT DO UPDATE SET
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
        .bind(&user.public_key_pem)
        .execute(self.db_connection())
        .await?;

        Ok(())
    }

    pub async fn get_user_by_id(&self, url: &Url) -> anyhow::Result<Option<User>> {
        let row = Self::select_user()
            .push("WHERE users.fed_id = $1")
            .build_query_as()
            .bind(url.as_str())
            .fetch_optional(self.db_connection())
            .await?;

        Ok(row)
    }

    pub async fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<User>> {
        let row = Self::select_user()
            .push("WHERE users.preferred_username = $1")
            .build_query_as()
            .bind(username)
            .fetch_optional(self.db_connection())
            .await?;

        Ok(row)
    }
}

impl FromRow<'_, SqliteRow> for User {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(User {
            id: ObjectId::parse(row.get("fed_id")).map_err(|x| sqlx::Error::ColumnDecode {
                index: "fed_id".into(),
                source: Box::new(x),
            })?,
            name: row.get("name"),
            preferred_username: row.get("preferred_username"),
            inbox: Url::parse(row.get("inbox")).map_err(|x| sqlx::Error::ColumnDecode {
                index: "inbox".into(),
                source: Box::new(x),
            })?,
            outbox: Url::parse(row.get("outbox")).map_err(|x| sqlx::Error::ColumnDecode {
                index: "outbox".into(),
                source: Box::new(x),
            })?,
            public_key_pem: row.get("public_key"),
            // TODO: Use this version for the pubkey
            // serde_json::from_str(row.get("public_key")).map_err(|x| {
            //     sqlx::Error::ColumnDecode {
            //         index: "public_key".into(),
            //         source: Box::new(x),
            //     }
            // })?,
            private_key_pem: row.get("private_key"),
            shared_inbox: row
                .get::<Option<&str>, _>("shared_inbox")
                .map(Url::parse)
                .transpose()
                .map_err(|x| sqlx::Error::ColumnDecode {
                    index: "outbox".into(),
                    source: Box::new(x),
                })?,
        })
    }
}
