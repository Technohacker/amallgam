use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::actor::PersonType,
    protocol::{public_key::PublicKey, verification::verify_domains_match},
    traits::{Actor, Object},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::context::AmallgamContext;

use super::User;

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

#[axum::async_trait]
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
        verify_domains_match(json.id.inner(), expected_domain)?;

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

        ctx.app_data().upsert_user(&user).await?;

        Ok(user)
    }
}
