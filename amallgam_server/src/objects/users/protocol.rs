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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolUser {
    pub id: ObjectId<User>,
    #[serde(rename = "type")]
    pub kind: PersonType,

    pub preferred_username: String,
    pub name: String,

    pub inbox: Url,
    pub outbox: Url,

    pub public_key: PublicKey,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Endpoints>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    pub shared_inbox: Url,
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
        let object_id: ObjectId<User> = ObjectId::from(object_id);

        if object_id.is_local(ctx) {
            // Local, grab the user ID
            let Some((_, user_id)) = object_id.inner().path().trim_end_matches('/').rsplit_once("/") else {
                return Err(anyhow::format_err!("Bad Local User URL?"));
            };

            ctx.get_bot_user_by_userid(user_id).await
        } else {
            // TODO: Use local cache
            Ok(None)
        }
    }

    async fn into_json(self, _ctx: &Data<Self::DataType>) -> Result<Self::Kind, Self::Error> {
        match &self {
            User::Local {
                base_url,
                user_id,
                display_name,
                ..
            } => Ok(ProtocolUser {
                id: base_url.clone().into(),
                kind: PersonType::Person,
                preferred_username: user_id.clone(),
                name: display_name.clone(),
                inbox: base_url.join("./inbox").expect("Bad Inbox URL?"),
                outbox: base_url.join("./outbox").expect("Bad Inbox URL?"),
                public_key: self.public_key(),
                endpoints: None,
            }),
            User::Remote(protocol_user) => Ok(*protocol_user.clone()),
        }
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
        let user = Self::Remote(Box::new(json));

        ctx.upsert_user(user.clone()).await?;

        Ok(user)
    }
}
