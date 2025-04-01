use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    traits::{ActivityHandler, Actor},
};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{activities::create::Create, context::AmallgamContext};

use super::notes::Note;

mod db;
mod protocol;

pub use protocol::ProtocolUser;

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

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
#[enum_delegate::implement(ActivityHandler)]
pub enum UserAllowedActivities {
    CreateNote(Create<Note>),
}

#[async_trait]
impl ActivityHandler for Create<Note> {
    type DataType = AmallgamContext;
    type Error = anyhow::Error;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, data: &Data<Self::DataType>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn receive(self, data: &Data<Self::DataType>) -> Result<(), Self::Error> {
        // log::info!("{:#?}", &self);

        Err(anyhow::format_err!("Temporary error to test receiving notes"))
    }
}
