use std::sync::Arc;

use activitypub_federation::{config::Data, traits::ActivityHandler};
use axum::async_trait;
use tokio::time::Instant;
use url::Url;

use crate::{AmallgamTesterContext, create::Create, note::Note};

#[async_trait]
impl ActivityHandler for Create<Note> {
    type DataType = Arc<AmallgamTesterContext>;
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

        ctx.note_ends.write().await.insert(
            note.in_reply_to.expect("Missing reply ID?").into_inner(),
            Instant::now(),
        );

        Ok(())
    }
}
