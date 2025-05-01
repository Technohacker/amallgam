use std::{path::PathBuf, sync::Arc, time::Duration};

use activitypub_federation::{
    activity_sending::SendActivityTask, config::Data, protocol::context::WithContext,
};
use anyhow::Result;
use llama_cpp::LlamaSession;
use model_config::LoadedModel;
use moka::future::Cache;
use reqwest_middleware::reqwest::Client;
use sqlx::{ConnectOptions, SqlitePool, sqlite::SqliteConnectOptions};
use tokio::{
    sync::{Mutex, RwLock, mpsc},
    task::JoinSet,
};
use url::Url;

use crate::{
    activities::{PendingActivity, create::Create},
    objects::{notes::Note, users::ProtocolUser},
};

mod llm;
mod model_config;
mod users;

pub use model_config::{ModelConfig, ModelId};
pub type ArcAmallgamContext = Arc<AmallgamContext>;

pub struct AmallgamConfig {
    /// Domain name to listen for federation requests on
    pub domain_name: String,

    /// URL to database
    pub db_url: Url,

    /// Path to models
    pub models_folder: PathBuf,

    /// Maximum number of LLM sessions that can run simultaneously
    pub max_simultaneous_sessions: usize,
    /// Number of CPU cores assigned for each session
    pub num_cores_per_session: u32,
}

pub struct AmallgamContext {
    config: AmallgamConfig,

    server_base_url: Url,

    pub(crate) remote_user_cache: Cache<Url, ProtocolUser>,
    pub(crate) db_connection: SqlitePool,

    /// Model ID -> Model
    llama_models: Cache<ModelId, LoadedModel>,
    /// Bot ID -> LLM Session
    llama_sessions: Cache<String, LlamaSession>,
    active_sessions_pool: Mutex<JoinSet<()>>,

    // For pending notes
    loopback_client: Client,
    note_sender: mpsc::Sender<PendingActivity<Create<Note>>>,
    pub(crate) pending_notes: RwLock<mpsc::Receiver<PendingActivity<Create<Note>>>>,
}

impl AmallgamContext {
    pub async fn new(config: AmallgamConfig) -> Result<ArcAmallgamContext> {
        let server_base_url =
            Url::parse(&format!("https://{}", &config.domain_name)).expect("Bad Server Base URL?");
        let db_connection = SqlitePool::connect_with(
            SqliteConnectOptions::from_url(&config.db_url)?.create_if_missing(true),
        )
        .await?;

        let (note_sender, pending_notes) = mpsc::channel(1024);

        let ctx = Self {
            config,
            server_base_url,

            remote_user_cache: Cache::new(1024),
            db_connection,

            llama_models: Cache::builder()
                .time_to_idle(Duration::from_secs(5 * 60))
                .build(),
            llama_sessions: Cache::builder()
                .time_to_idle(Duration::from_secs(5 * 60))
                .build(),
            active_sessions_pool: Mutex::new(JoinSet::new()),

            loopback_client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Couldn't build loopback client?"),
            note_sender,
            pending_notes: RwLock::new(pending_notes),
        };

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        Ok(Arc::new(ctx))
    }

    pub(crate) fn server_relative_url(&self, suffix: &str) -> Url {
        self.server_base_url
            .join(suffix)
            .expect("Bad Server-relative URL?")
    }

    /// Returns if the queue up was successful
    #[must_use = "If the queue is full, this will not run any operation"]
    pub(crate) async fn try_queue_up_pending_note(
        self: &Arc<Self>,
        future: impl Future<Output = Result<PendingActivity<Create<Note>>>> + Send + 'static,
    ) -> bool {
        log::info!("Checking for free session...");
        let mut pool_lock = self.active_sessions_pool.lock().await;

        // Check if we're above the number of active sessions
        if pool_lock.len() < self.config.max_simultaneous_sessions {
            // If we've got free space, we can queue up normally. Keep the pool locked until we're done
        } else {
            // If not, check if a session is done
            let try_free = pool_lock.try_join_next();
            if try_free.is_none() {
                // This option is None only if there were no completed tasks or if the queue is empty
                // We make sure it's not the empty path with the check above

                // There are no free slots available. Bail out
                log::info!("Free session not available");
                return false;
            }

            // If we're here, there was a free slot
            log::info!("Free session available");
        }

        // The lock is still held here, so we use it to queue up the task

        let ctx = self.clone();
        pool_lock.spawn(async move {
            let res = async {
                let activity = future.await?;

                let loopback_url = ctx.server_relative_url("./_internal/run_pending_notes");

                ctx.note_sender.send(activity).await?;
                ctx.loopback_client.post(loopback_url).send().await?;

                anyhow::Ok(())
            };

            if let Err(err) = res.await {
                log::warn!("Pending activity send failed! {}", err);
            }
        });

        // And since we were able to queue one up, signal it to the user
        true
    }

    pub(crate) async fn send_pending_note_immediately(
        data_ctx: &Data<ArcAmallgamContext>,
        pending: PendingActivity<Create<Note>>,
    ) -> Result<()> {
        let bot_user = pending
            .activity
            .actor
            .dereference_local(data_ctx)
            .await
            .expect("Missing bot user?");

        let msg = WithContext::new_default(pending.activity);

        let sends =
            SendActivityTask::prepare(&msg, &bot_user, pending.target_inboxes.clone(), data_ctx)
                .await?;

        futures::future::try_join_all(
            sends
                .into_iter()
                .map(|x| async move { x.sign_and_send(data_ctx).await }),
        )
        .await?;

        Ok(())
    }
}
