use std::{path::PathBuf, sync::Arc, time::Duration};

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

    pub(crate) async fn queue_up_pending_note(
        &self,
        future: impl Future<Output = Result<PendingActivity<Create<Note>>>> + Send + 'static,
    ) {
        let sender = self.note_sender.clone();
        let loopback_url = self.server_relative_url("./_internal/run_pending_notes");
        let client = self.loopback_client.clone();

        // Check if we're above the number of active sessions
        let mut pool_lock = loop {
            // First get a lock on the pool
            log::info!("Checking for free session...");
            let mut pool_lock = self.active_sessions_pool.lock().await;

            // Check how many active sessions are going on
            if pool_lock.len() < self.config.max_simultaneous_sessions {
                // If we've got free space, use the lock in the outside code
                break pool_lock;
            } else {
                // If not, wait for a session to be done
                // This will keep the mutex locked, forcing future requests to be held up by the pool lock above
                log::info!("Free session not available. Waiting...");
                pool_lock.join_next().await;
            }
        };

        log::info!("Free session available");
        pool_lock.spawn(async move {
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
