use std::sync::Arc;

use anyhow::Result;
use llama_cpp::{LlamaModel, LlamaParams, LlamaSession, SessionParams};
use moka::future::Cache;
use reqwest_middleware::reqwest::Client;
use sqlx::SqlitePool;
use tokio::sync::{RwLock, mpsc};
use url::Url;

use crate::{
    activities::{PendingActivity, create::Create},
    objects::{notes::Note, users::ProtocolUser},
};

pub type ArcAmallgamContext = Arc<AmallgamContext>;

pub struct AmallgamContext {
    server_base_url: Url,

    pub(crate) remote_user_cache: Cache<Url, ProtocolUser>,
    db_connection: SqlitePool,

    pub(crate) llama_context: RwLock<LlamaSession>,

    loopback_client: Client,
    pub(crate) note_sender: mpsc::Sender<PendingActivity<Create<Note>>>,
    pub(crate) pending_notes: RwLock<mpsc::Receiver<PendingActivity<Create<Note>>>>,
}

impl AmallgamContext {
    pub async fn new(server_base_url: Url, db_url: &str) -> Result<ArcAmallgamContext> {
        let (note_sender, pending_notes) = mpsc::channel(1024);

        let ctx = AmallgamContext {
            server_base_url,

            remote_user_cache: Cache::new(1024),
            db_connection: SqlitePool::connect(db_url).await?,

            llama_context: RwLock::new(
                LlamaModel::load_from_file(
                    "/amallgam/models/phi-1_5-q4_k_m.gguf",
                    LlamaParams {
                        ..Default::default()
                    },
                )?
                .create_session(SessionParams {
                    n_threads: 4,
                    ..Default::default()
                })?,
            ),

            loopback_client: Client::builder().danger_accept_invalid_certs(true).build()?,
            note_sender,
            pending_notes: RwLock::new(pending_notes),
        };

        ctx.llama_context.write().await.advance_context(
            "You are a helpful assistant. A user has a question or needs assistance with a task.\n\nUser: ",
        )?;

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        // TODO: Remove this temporary user
        let user = ctx.new_bot_user("def");
        ctx.upsert_user(user).await.unwrap();

        Ok(Arc::new(ctx))
    }

    pub(crate) fn server_relative_url(&self, suffix: &str) -> Url {
        self.server_base_url
            .join(suffix)
            .expect("Bad Server-relative URL?")
    }

    pub(crate) fn db_connection(&self) -> &SqlitePool {
        &self.db_connection
    }

    pub(crate) fn queue_up_pending_note(
        &self,
        future: impl Future<Output = Result<PendingActivity<Create<Note>>>> + Send + 'static,
    ) {
        let sender = self.note_sender.clone();
        let loopback_url = self.server_relative_url("./_internal/run_pending_notes");
        let client = self.loopback_client.clone();

        tokio::spawn(async move {
            let res = async {
                let activity = future.await?;
                sender.send(activity).await?;

                client.post(loopback_url).send().await?;
                anyhow::Ok(())
            };

            if let Err(err) = res.await {
                log::warn!("Pending activity send failed! {}", err);
            }
        });
    }
}
