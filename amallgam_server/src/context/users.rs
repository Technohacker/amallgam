use activitypub_federation::http_signatures;
use sqlx::{Row, sqlite::SqliteRow};
use url::Url;

use crate::{context::AmallgamContext, objects::users::User};

use super::ModelId;

impl AmallgamContext {
    fn user_base_url(&self, user_id: &str) -> Url {
        self.server_relative_url(&format!("./user/{user_id}/"))
    }

    pub fn new_bot_user(&self, user_id: &str, model_id: ModelId, system_prompt: impl Into<String>) -> User {
        let kp = http_signatures::generate_actor_keypair().expect("Failed to generate KeyPair?");

        User::Local {
            base_url: self.user_base_url(user_id),

            user_id: user_id.to_string(),
            display_name: user_id.to_string(),

            public_key_pem: kp.public_key,
            private_key_pem: kp.private_key,

            model_id,
            system_prompt: system_prompt.into(),
        }
    }

    pub async fn upsert_user(&self, user: User) -> anyhow::Result<()> {
        match user {
            User::Remote(protocol_user) => {
                // Remote users are kept in cache
                self.remote_user_cache
                    .insert(
                        protocol_user.id.inner().clone(),
                        protocol_user,
                    )
                    .await;
            }
            User::Local {
                base_url: _,
                user_id,
                display_name,
                public_key_pem,
                private_key_pem,
                model_id,
                system_prompt,
            } => {
                // Local users are persisted

                sqlx::query(
                    "
                    INSERT INTO bot_users (
                        id,
                        display_name,
                        private_key,
                        public_key,
                        model_id,
                        system_prompt
                    ) VALUES (
                        $1,
                        $2,
                        $3,
                        $4,
                        $5,
                        $6
                    ) ON CONFLICT DO UPDATE SET
                        display_name = $2,
                        private_key = $3,
                        public_key = $4,
                        model_id = $5,
                        system_prompt = $6
                    ",
                )
                .bind(user_id)
                .bind(display_name)
                .bind(private_key_pem)
                .bind(public_key_pem)
                .bind(model_id.0)
                .bind(system_prompt)
                .execute(&self.db_connection)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn get_bot_user_by_userid(&self, user_id: &str) -> anyhow::Result<Option<User>> {
        let row = sqlx::query(
            "
            SELECT
                id,
                display_name,
                public_key,
                private_key,
                model_id,
                system_prompt
            FROM bot_users
            WHERE id = $1
            ",
        )
        .bind(user_id)
        .map(|x| self.bot_user_from_row(x))
        .fetch_optional(&self.db_connection)
        .await?;

        Ok(row)
    }

    fn bot_user_from_row(&self, row: SqliteRow) -> User {
        let user_id = row.get("id");

        User::Local {
            base_url: self.user_base_url(user_id),
            user_id: user_id.to_string(),
            display_name: row.get("display_name"),
            public_key_pem: row.get("public_key"),
            private_key_pem: row.get("private_key"),
            model_id: ModelId(row.get("model_id")),
            system_prompt: row.get("system_prompt"),
        }
    }
}
