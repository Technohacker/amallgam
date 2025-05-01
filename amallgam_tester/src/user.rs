use std::sync::Arc;

use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::actor::PersonType,
    protocol::public_key::PublicKey,
    traits::{Actor, Object},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::AmallgamTesterContext;

/// User data sent over the protocol
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: ObjectId<User>,
    #[serde(rename = "type")]
    pub kind: PersonType,

    pub preferred_username: String,
    pub name: String,

    pub inbox: Url,
    pub outbox: Url,

    pub public_key: PublicKey,
    #[serde(skip)]
    pub private_key: Option<String>,
}

#[axum::async_trait]
impl Object for User {
    type DataType = Arc<AmallgamTesterContext>;
    type Kind = Self;
    type Error = anyhow::Error;

    async fn read_from_id(
        object_id: Url,
        ctx: &Data<Self::DataType>,
    ) -> Result<Option<Self>, Self::Error> {
        // May be local or remote
        let object_id: ObjectId<User> = ObjectId::from(object_id);

        if object_id == ctx.user.id {
            Ok(Some(ctx.user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn into_json(self, _ctx: &Data<Self::DataType>) -> Result<Self::Kind, Self::Error> {
        Ok(self)
    }

    async fn verify(
        _json: &Self::Kind,
        _expected_domain: &Url,
        _ctx: &Data<Self::DataType>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn from_json(json: Self::Kind, _ctx: &Data<Self::DataType>) -> Result<Self, Self::Error> {
        Ok(json)
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
