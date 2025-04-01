use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::{link::MentionType, object::NoteType},
    protocol::{helpers::deserialize_one_or_many, verification::verify_domains_match},
    traits::Object,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{context::AmallgamContext, objects::users::User};

/// Note data
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: ObjectId<Note>,
    #[serde(rename = "type")]
    pub kind: NoteType,

    pub attributed_to: ObjectId<User>,
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub to: Vec<Url>,
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub cc: Vec<Url>,
    pub content: String,

    // #[serde(deserialize_with = "dese")]
    // TODO: Handle multiple values a la Lemmy
    pub in_reply_to: Option<ObjectId<Note>>,

    pub tag: Vec<Mention>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mention {
    pub href: Url,
    #[serde(rename = "type")]
    pub kind: MentionType,
}

#[axum::async_trait]
impl Object for Note {
    type DataType = AmallgamContext;
    type Kind = Self;
    type Error = anyhow::Error;

    async fn read_from_id(
        object_id: Url,
        ctx: &Data<Self::DataType>,
    ) -> Result<Option<Self>, Self::Error> {
        // May be local or remote
        Ok(None)
    }

    async fn into_json(self, _ctx: &Data<Self::DataType>) -> Result<Self::Kind, Self::Error> {
        Ok(self)
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
        // Only called for remote notes
        Ok(json)
    }
}
