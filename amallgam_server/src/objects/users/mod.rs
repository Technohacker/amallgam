use activitypub_federation::{
    config::Data,
    http_signatures,
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

/// User data
#[derive(Debug, Clone)]
pub enum User {
    Local {
        base_url: Url,

        user_id: String,
        display_name: String,

        public_key_pem: String,
        private_key_pem: String,
    },
    Remote(Box<ProtocolUser>),
}

impl AmallgamContext {
    fn user_base_url(&self, user_id: &str) -> Url {
        self.server_base_url()
            .join(&format!("/user/{user_id}"))
            .expect("Bad URL for user?")
    }

    pub fn new_bot_user(&self, user_id: &str) -> User {
        let kp = http_signatures::generate_actor_keypair().expect("Failed to generate KeyPair?");

        User::Local {
            base_url: self.user_base_url(user_id),

            user_id: user_id.to_string(),
            display_name: user_id.to_string(),

            public_key_pem: kp.public_key,
            private_key_pem: kp.private_key,
        }
    }
}

impl Actor for User {
    fn id(&self) -> Url {
        match self {
            User::Local { base_url, .. } => base_url,
            User::Remote(protocol_user) => protocol_user.id.inner(),
        }
        .clone()
    }

    fn public_key_pem(&self) -> &str {
        match self {
            User::Local { public_key_pem, .. } => public_key_pem,
            User::Remote(protocol_user) => &protocol_user.public_key.public_key_pem,
        }
    }

    fn private_key_pem(&self) -> Option<String> {
        match self {
            User::Local {
                private_key_pem, ..
            } => Some(private_key_pem.clone()),
            User::Remote(_) => None,
        }
    }

    fn inbox(&self) -> Url {
        match self {
            User::Local { base_url, .. } => base_url.join("/inbox").expect("Bad Inbox URL?"),
            User::Remote(protocol_user) => protocol_user.inbox.clone(),
        }
    }

    fn shared_inbox(&self) -> Option<Url> {
        match self {
            User::Local { .. } => None,
            User::Remote(protocol_user) => protocol_user
                .endpoints
                .as_ref()
                .map(|x| x.shared_inbox.clone()),
        }
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

        Err(anyhow::format_err!(
            "Temporary error to test receiving notes"
        ))
    }
}
