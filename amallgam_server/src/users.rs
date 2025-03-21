use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::actor::PersonType,
    protocol::public_key::PublicKey,
    traits::{Actor, Object},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, sqlite::SqliteRow};
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

    pub public_key: PublicKey,
    pub private_key: Option<String>,
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
            public_key: PublicKey {
                id: row.get("fed_id"),
                owner: Url::parse("http://example.com/user/abc").expect("A"),
                public_key_pem: String::new(),
            },
            // TODO: Use this version for the pubkey
            // serde_json::from_str(row.get("public_key")).map_err(|x| {
            //     sqlx::Error::ColumnDecode {
            //         index: "public_key".into(),
            //         source: Box::new(x),
            //     }
            // })?,
            private_key: row.get("preferred_username"),
        })
    }
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
            preferred_username: self.preferred_username,
            name: self.name,
            inbox: self.inbox,
            outbox: self.outbox,
            public_key: self.public_key,
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
            public_key: json.public_key,
            private_key: None,
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
        &self.public_key.public_key_pem
    }

    fn private_key_pem(&self) -> Option<String> {
        self.private_key.clone()
    }

    fn inbox(&self) -> Url {
        self.inbox.clone()
    }
}
