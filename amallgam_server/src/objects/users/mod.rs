use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::public,
    traits::{ActivityHandler, Actor},
};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    activities::{create::Create, PendingActivity},
    context::{AmallgamContext, ArcAmallgamContext, ModelId},
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
        let note = &self.object;
        log::info!("Note Received: {}", &note.id);

        let create_sender_id = &self.actor;
        let note_sender_id = &self.object.attributed_to;
        let note_sender = note_sender_id.dereference(ctx).await?;

        // Check if this is an alias offload request
        let offload_request = create_sender_id != note_sender_id;

        let mentioned_bots = if offload_request {
            // This is an offload request, check if we have the bot
            log::info!("Request is an alias offload for: {}", create_sender_id);

            let Some(offload_bot) = ctx.find_bot_for_alias(create_sender_id).await? else {
                log::info!("Offload bot not found: {}", create_sender_id);
                return Err(anyhow::format_err!(
                    "Offload bot not found for: {}",
                    create_sender_id
                ));
            };

            // And only process it with the offload bot
            vec![offload_bot]
        } else {
            // This is a normal request. Find the mentioned bots
            let mut bots = vec![];

            for mention in &note.tag {
                let bot_id = &mention.href;

                // Skip remote mentions
                if !bot_id.is_local(ctx) {
                    continue;
                }
                log::info!("\tBot Mentioned: {}", &bot_id);

                let Ok(bot) = bot_id.dereference_local(ctx).await else {
                    log::warn!("\tBot Missing: {}", &bot_id);
                    continue;
                };

                bots.push(bot);
            }

            bots
        };

        for bot in mentioned_bots {
            let arc_ctx = ctx.app_data().clone();

            let bot_id = ObjectId::from(bot.id());
            let note = note.clone();

            let note_sender_id = note_sender_id.clone();

            if offload_request {
                // Compute the offload response
                let target_inboxes = vec![note_sender.shared_inbox_or_inbox()];

                ctx.queue_up_pending_note(async move {
                    let message = note.content;
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
                                vec![note_sender_id.inner().clone()],
                                completion,
                                Some(note.id),
                                [Mention::for_user(note_sender_id.clone())],
                            ),
                        ),
                        target_inboxes,
                    })
                })
                .await;
            } else {
                // Test out offloading
                log::info!("Offloading note: {}", &note.id);
                let User::Local { user_id, .. } = bot else {
                    panic!("Attempted to use remote user as a bot");
                };

                let aliases = arc_ctx.get_bot_aliases(&user_id).await?;
                let mut target_inboxes = vec![];
                for alias in aliases {
                    match alias.dereference(ctx).await {
                        Ok(alias) => {
                            target_inboxes.push(alias.shared_inbox_or_inbox());
                        }
                        Err(err) => {
                            log::info!("Offload bot not found: {} {}", &alias, err);
                        }
                    }
                }

                AmallgamContext::send_pending_note_immediately(ctx, PendingActivity {
                    activity: arc_ctx.new_create_activity(
                        bot_id.clone(),
                        vec![public()],
                        vec![],
                        note,
                    ),
                    target_inboxes,
                })
                .await?;
            }
        }

        Ok(())
    }
}
