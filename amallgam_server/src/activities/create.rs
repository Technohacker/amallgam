use activitypub_federation::{fetch::object_id::ObjectId, kinds::activity::CreateType, protocol::helpers::deserialize_one_or_many};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::objects::users::User;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Create<T> {
    pub id: Url,
    #[serde(rename = "type")]
    pub kind: CreateType,

    pub actor: ObjectId<User>,

    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub to: Vec<Url>,
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub cc: Vec<Url>,

    // #[serde(skip)]
    // #[serde(default = "None")]
    pub object: T,
}
