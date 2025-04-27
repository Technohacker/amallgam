use activitypub_federation::{
    config::Data,
    kinds::public,
    traits::{ActivityHandler, Actor},
};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    activities::{create::Create, PendingActivity},
    context::{ArcAmallgamContext, ModelId},
};

use super::notes::{Mention, Note};

mod protocol;

pub use self::protocol::ProtocolUser;

/// User data
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum User {
    Local {
        base_url: Url,

        user_id: String,
        display_name: String,

        public_key_pem: String,
        private_key_pem: String,

        model_id: ModelId,
        system_prompt: String,
    },
    Remote(ProtocolUser),
}

impl User {
    pub fn display_name(&self) -> &str {
        match self {
            User::Local { display_name, .. } => display_name,
            User::Remote(protocol_user) => &protocol_user.name,
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
            User::Local { base_url, .. } => base_url.join("./inbox").expect("Bad Inbox URL?"),
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
    type DataType = ArcAmallgamContext;
    type Error = anyhow::Error;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, _ctx: &Data<Self::DataType>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn receive(self, ctx: &Data<Self::DataType>) -> Result<(), Self::Error> {
        let note_id = &self.object.id;
        log::info!("Note Received: {}", note_id);

        let sender_id = &self.object.attributed_to;
        let sender = sender_id.dereference(ctx).await?;
        let target_inboxes = vec![sender.shared_inbox_or_inbox()];

        let mentioned_bots = self.object.tag.iter().filter(|x| x.href.is_local(ctx));

        for mention in mentioned_bots {
            let bot_id = mention.href.clone();
            log::info!("\tBot Mentioned: {}", &bot_id);

            let Ok(bot) = bot_id.dereference_local(ctx).await else {
                log::warn!("\tBot Missing: {}", &bot_id);
                continue;
            };

            let arc_ctx = ctx.app_data().clone();

            let note_id = note_id.clone();

            let sender_id = sender_id.clone();
            let target_inboxes = target_inboxes.clone();

            let message = self.object.content.clone();

            ctx.queue_up_pending_note(async move {
                log::info!("Received Message: {message}");
                let completion = arc_ctx.run_llm_inference(&bot, message).await?;

                Ok(PendingActivity {
                    activity: arc_ctx.new_create_activity(
                        bot_id.clone(),
                        vec![public()],
                        vec![],
                        arc_ctx.new_note(
                            bot_id.clone(),
                            vec![public()],
                            vec![sender_id.inner().clone()],
                            completion,
                            Some(note_id),
                            [Mention::for_user(sender_id.clone())],
                        ),
                    ),
                    target_inboxes,
                })
            })
            .await;
        }

        Ok(())
    }
}
