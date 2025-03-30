use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::actor::PersonType,
    protocol::public_key::PublicKey,
    traits::{Actor, Object},
};
use serde::{Deserialize, Serialize};
use sqlx::{Database, FromRow, QueryBuilder, Row, sqlite::SqliteRow};
use url::Url;

use crate::context::AmallgamContext;

/// User data stored persistently
#[derive(Debug, Clone)]
pub struct User {
    pub id: ObjectId<User>,
    pub name: String,
    pub preferred_username: String,

    pub inbox: Url,
    pub outbox: Url,

    pub public_key_pem: String,
    pub private_key_pem: Option<String>,

    pub shared_inbox: Option<Url>,
}

/// User data sent over the protocol
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolUser {
    id: ObjectId<User>,
    #[serde(rename = "type")]
    kind: PersonType,

    preferred_username: String,
    name: String,

    inbox: Url,
    outbox: Url,

    public_key: PublicKey,

    #[serde(skip_serializing_if = "Option::is_none")]
    endpoints: Option<Endpoints>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    shared_inbox: Url,
}

#[async_trait::async_trait]
impl Object for User {
    type DataType = AmallgamContext;
    type Kind = ProtocolUser;
    type Error = anyhow::Error;

    async fn read_from_id(
        object_id: Url,
        ctx: &Data<Self::DataType>,
    ) -> Result<Option<Self>, Self::Error> {
        // May be local or remote
        ctx.get_user_by_id(&object_id).await
    }

    async fn into_json(self, _ctx: &Data<Self::DataType>) -> Result<Self::Kind, Self::Error> {
        Ok(Self::Kind {
            id: self.id.clone(),
            kind: PersonType::Person,
            preferred_username: self.preferred_username.clone(),
            name: self.name.clone(),
            inbox: self.inbox.clone(),
            outbox: self.outbox.clone(),
            public_key: self.public_key(),
            endpoints: self
                .shared_inbox
                .map(|shared_inbox| Endpoints { shared_inbox }),
        })
    }

    async fn verify(
        json: &Self::Kind,
        expected_domain: &Url,
        _ctx: &Data<Self::DataType>,
    ) -> Result<(), Self::Error> {
        if json.id.inner().host() != expected_domain.host() {
            return Err(anyhow::format_err!("ID Domain does not match!"));
        }

        Ok(())
    }

    async fn from_json(json: Self::Kind, ctx: &Data<Self::DataType>) -> Result<Self, Self::Error> {
        // Only called for remote users
        let user = Self {
            id: json.id,
            preferred_username: json.preferred_username,
            name: json.name,
            inbox: json.inbox,
            outbox: json.outbox,
            public_key_pem: json.public_key.public_key_pem,
            private_key_pem: None,
            shared_inbox: json.endpoints.map(|x| x.shared_inbox),
        };

        ctx.app_data().upsert_user(user.clone()).await?;

        Ok(user)
    }
}

impl Actor for User {
    fn id(&self) -> Url {
        self.id.inner().clone()
    }

    fn public_key_pem(&self) -> &str {
        &self.public_key_pem
    }

    fn private_key_pem(&self) -> Option<String> {
        self.private_key_pem.clone()
    }

    fn inbox(&self) -> Url {
        self.inbox.clone()
    }

    fn shared_inbox(&self) -> Option<Url> {
        self.shared_inbox.clone()
    }
}

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

    pub async fn upsert_user(&self, user: User) -> anyhow::Result<()> {
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
            .push("WHERE id = $1")
            .build_query_as()
            .bind(url.as_str())
            .fetch_optional(self.db_connection())
            .await?;

        Ok(row)
    }

    pub async fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<User>> {
        let row = Self::select_user()
            .push("WHERE preferred_username = $1")
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
