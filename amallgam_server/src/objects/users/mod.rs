use activitypub_federation::{
    config::Data, fetch::object_id::ObjectId, http_signatures, traits::{ActivityHandler, Actor}
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
    id: ObjectId<User>,
    name: String,
    preferred_username: String,

    inbox: Url,
    outbox: Url,

    public_key_pem: String,
    private_key_pem: Option<String>,

    shared_inbox: Option<Url>,
}

impl User {
    pub fn new(user_id: &str) -> Self {
        let kp = http_signatures::generate_actor_keypair().expect("Failed to generate KeyPair?");

        let url_base = format!("https://amallgam.docker/user/{user_id}");

        Self {
            id: url_base.parse().unwrap(),
            preferred_username: user_id.into(),
            name: user_id.into(),
            inbox: format!("{url_base}/inbox").parse().unwrap(),
            outbox: format!("{url_base}/outbox").parse().unwrap(),
            public_key_pem: kp.public_key,
            private_key_pem: Some(kp.private_key),
            shared_inbox: None,
        }
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
