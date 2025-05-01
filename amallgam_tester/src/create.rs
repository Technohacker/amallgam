use activitypub_federation::{
    fetch::object_id::ObjectId, kinds::activity::CreateType,
    protocol::helpers::deserialize_one_or_many,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{user::User, AmallgamTesterContext};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Create<T> {
    pub id: Url,
    #[serde(rename = "type")]
    kind: CreateType,

    pub actor: ObjectId<User>,

    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub to: Vec<Url>,
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub cc: Vec<Url>,

    pub object: T,
}

impl AmallgamTesterContext {
    pub fn new_create_activity<T>(
        &self,
        actor: ObjectId<User>,
        to: impl Into<Vec<Url>>,
        cc: impl Into<Vec<Url>>,
        object: T,
    ) -> Create<T> {
        Create {
            id: self.base_url.join(&format!("./creates/{}", Uuid::now_v7())).expect("Bad URL?"),
            kind: CreateType::Create,
            actor,
            to: to.into(),
            cc: cc.into(),
            object,
        }
    }
}
