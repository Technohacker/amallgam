use sqlx::{Database, QueryBuilder};

pub fn select_user<'args, DB: Database>() -> QueryBuilder<'args, DB> {
    QueryBuilder::new(
        "
            SELECT
                users.fed_id as fed_id,
                users.preferred_username,
                users.name,
                users.inbox,
                users.outbox,
                users.public_key,
                bot_users.private_key
            FROM users
            LEFT JOIN bot_users
                ON users.fed_id = bot_users.fed_id
            "
    )
}
