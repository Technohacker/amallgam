use sqlx::{Row, sqlite::SqliteRow};

use crate::context::AmallgamContext;

use super::User;

impl AmallgamContext {
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
                model_name,
                system_prompt,
            } => {
                // Local users are persisted

                sqlx::query(
                    "
                    INSERT INTO bot_users (
                        user_id,
                        display_name,
                        private_key,
                        public_key,
                        model_name,
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
                        model_name = $5,
                        system_prompt = $6
                    ",
                )
                .bind(user_id)
                .bind(display_name)
                .bind(private_key_pem)
                .bind(public_key_pem)
                .bind(model_name)
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
                user_id,
                display_name,
                public_key,
                private_key,
                model_name,
                system_prompt
            FROM bot_users
            WHERE user_id = $1
            ",
        )
        .bind(user_id)
        .map(|x| self.bot_user_from_row(x))
        .fetch_optional(&self.db_connection)
        .await?;

        Ok(row)
    }

    fn bot_user_from_row(&self, row: SqliteRow) -> User {
        let user_id = row.get("user_id");

        User::Local {
            base_url: self.user_base_url(user_id),
            user_id: user_id.to_string(),
            display_name: row.get("display_name"),
            public_key_pem: row.get("public_key"),
            private_key_pem: row.get("private_key"),
            model_name: row.get("model_name"),
            system_prompt: row.get("system_prompt"),
        }
    }
}
