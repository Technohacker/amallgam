use std::sync::Arc;

use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::public,
    traits::{ActivityHandler, Actor},
};
use anyhow::Result;
use axum::async_trait;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    activities::{PendingActivity, create::Create},
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
        let note = self.object;
        log::info!("Note Received: {}", &note.id);

        let create_sender_id = &self.actor;
        let note_sender_id = &note.attributed_to;
        let note_sender = note_sender_id.dereference(ctx).await?;

        // Check if this is an alias offload request
        let offload_request = create_sender_id != note_sender_id;

        if offload_request {
            AmallgamContext::handle_offload_note(ctx, note, note_sender, create_sender_id).await?;
        } else {
            AmallgamContext::handle_normal_note(ctx, note, note_sender).await?;
        };

        Ok(())
    }
}

impl AmallgamContext {
    async fn handle_normal_note(
        ctx: &Data<ArcAmallgamContext>,
        note: Note,
        note_sender: User,
    ) -> Result<()> {
        // This is a normal request. Take the first locally mentioned bot
        let mut mentioned_bot = None;
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

            // Take the mentioned bot and continue
            mentioned_bot = Some(bot);
            break;
        };

        // If there weren't any bots, bail early
        let Some(mentioned_bot) = mentioned_bot else {
            // Ok return to avoid retries from the client
            return Ok(());
        };

        let arc_ctx = ctx.app_data().clone();

        // Try to queue it up
        let queued_successfully = arc_ctx.try_process_note(note.clone(), note_sender, mentioned_bot.clone()).await?;

        if queued_successfully {
            // If we've queued up successfully, return success
            Ok(())
        } else {
            // The queue was full, this note should be offloaded
            log::info!("Queue full. Offloading note: {}...", &note.id);

            // Test out offloading
            let User::Local { user_id, .. } = &mentioned_bot else {
                panic!("Attempted to use remote user as a bot");
            };

            // Get all aliases and shuffle them
            let mut aliases = arc_ctx.get_bot_aliases(user_id).await?;
            aliases.shuffle(&mut rand::rng());

            // Try each alias one-by-one
            for alias in aliases {
                let offload_bot = match alias.dereference(ctx).await {
                    Ok(alias) => {
                        alias
                    }
                    Err(err) => {
                        log::info!("Offload bot not found: {} {}", &alias, err);
                        continue;
                    }
                };

                // Send the offload activity immediately. This will fail if the alias is also busy
                let res = Self::send_pending_note_immediately(
                    ctx,
                    PendingActivity {
                        activity: arc_ctx.new_create_activity(
                            ObjectId::from(mentioned_bot.id()),
                            vec![public()],
                            vec![],
                            note.clone(),
                        ),
                        target_inboxes: vec![offload_bot.shared_inbox_or_inbox()],
                    },
                )
                .await;

                match res {
                    Ok(_) => {
                        // The alias has queued up successfully. Return success
                        return Ok(());
                    },
                    Err(err) => {
                        // We weren't able to queue up on the alias. Try the next one
                        log::warn!("Offload failed for alias: {err}");
                        continue;
                    },
                }
            }

            // If we're here, all of the queues were unavailable. Signal it to the client
            Err(anyhow::format_err!("Resources unavailable. Try again"))
        }
    }

    async fn handle_offload_note(
        ctx: &Data<ArcAmallgamContext>,
        note: Note,
        note_sender: User,
        offloaded_from: &ObjectId<User>,
    ) -> Result<()> {
        // This is an offload request, check if we have the bot
        log::info!("Request is an alias offload for: {}", offloaded_from);

        let Some(offload_bot) = ctx.find_bot_for_alias(offloaded_from).await? else {
            log::warn!("Offload bot not found: {}", offloaded_from);
            return Err(anyhow::format_err!(
                "Offload bot not found for: {}",
                offloaded_from
            ));
        };

        log::info!("Offloading with: {}", offload_bot.id());
        let queued_successfully = ctx.try_process_note(note, note_sender, offload_bot).await?;

        // If we weren't able to queue it up, bail and let the original bot handle subsequent tries
        if queued_successfully {
            Ok(())
        } else {
            Err(anyhow::format_err!("Session queue full"))
        }
    }

    #[must_use = "If the queue is full, this will not run any operation"]
    async fn try_process_note(
        self: &Arc<Self>,
        note: Note,
        note_sender: User,
        bot: User,
    ) -> Result<bool> {
        let bot_id = ObjectId::from(bot.id());
        let note_sender_id = note.attributed_to;

        let message = note.content;

        let arc_ctx = self.clone();
        let queued_successfully = self
            .try_queue_up_pending_note(async move {
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
                            [Mention::for_user(note_sender_id)],
                        ),
                    ),
                    target_inboxes: vec![note_sender.shared_inbox_or_inbox()],
                })
            })
            .await;

        Ok(queued_successfully)
    }
}
