use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use llama_cpp::{LlamaModel, LlamaParams, LlamaSession, SessionParams};
use moka::future::Cache;
use reqwest_middleware::reqwest::Client;
use sqlx::{ConnectOptions, SqlitePool, sqlite::SqliteConnectOptions};
use tokio::sync::{RwLock, mpsc};
use url::Url;

use crate::{
    activities::{PendingActivity, create::Create},
    objects::{
        notes::Note,
        users::{ProtocolUser, User},
    },
};

pub type ArcAmallgamContext = Arc<AmallgamContext>;

pub struct AmallgamConfig {
    /// Domain name to listen for federation requests on
    pub domain_name: String,

    /// URL to database
    pub db_url: Url,

    /// Path to models
    pub models_folder: PathBuf,
}

pub struct AmallgamContext {
    config: AmallgamConfig,

    server_base_url: Url,

    pub(crate) remote_user_cache: Cache<Url, ProtocolUser>,
    pub(crate) db_connection: SqlitePool,

    llama_models: Cache<PathBuf, LlamaModel>,
    llama_sessions: Cache<String, LlamaSession>,

    // For pending notes
    loopback_client: Client,
    pub(crate) note_sender: mpsc::Sender<PendingActivity<Create<Note>>>,
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

            loopback_client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()?,
            note_sender,
            pending_notes: RwLock::new(pending_notes),
        };

        sqlx::migrate!("db/migrations")
            .run(&ctx.db_connection)
            .await?;

        // TODO: Remove this temporary user
        let user = ctx.new_bot_user(
            "def",
            "qwen1.5-0.5b-chat-q4_k_m.gguf",
            "You are a helpful AI assistant. The following is a tweet from a user. Write a reply tweet."
        );
        ctx.upsert_user(user).await.unwrap();

        Ok(Arc::new(ctx))
    }

    pub(crate) fn server_relative_url(&self, suffix: &str) -> Url {
        self.server_base_url
            .join(suffix)
            .expect("Bad Server-relative URL?")
    }

    pub(crate) async fn run_llm_inference(
        &self,
        bot_user: &User,
        sender_name: impl AsRef<str>,
        message: impl AsRef<str>,
    ) -> Result<String> {
        let User::Local {
            user_id,
            model_name,
            system_prompt,
            ..
        } = bot_user
        else {
            return Err(anyhow::format_err!(
                "Attempted to use a remote user as a bot"
            ));
        };
        let model_path = self.config.models_folder.join(model_name);

        let mut session = self
            .llama_sessions
            .try_get_with_by_ref(user_id, async {
                let model = self
                    .llama_models
                    .try_get_with_by_ref(
                        &model_path,
                        LlamaModel::load_from_file_async(
                            &model_path,
                            LlamaParams {
                                n_gpu_layers: 0,
                                ..Default::default()
                            },
                        ),
                    )
                    .await?;

                let mut session = model.create_session(SessionParams {
                    n_threads: num_cpus::get().min(u32::MAX as usize) as u32,
                    ..Default::default()
                })?;

                session
                    .set_context_to_tokens_async(session.model().tokenize_bytes(
                        format!(
                            "<|im_start|>system\n{system_prompt}<|im_end|>\n<|im_start|>user\n"
                        ),
                        false,
                        true,
                    )?)
                    .await?;

                anyhow::Ok(session)
            })
            .await
            .and_then(|x| {
                Ok(x.deep_copy().map_err(|err| {
                    anyhow::format_err!("Error occured when cloning session: {err}")
                })?)
            })
            .map_err(|err| {
                anyhow::format_err!("Error occured when preparing LLM Session: {err}")
            })?;

        let sender_name = sender_name.as_ref();
        let mut prompt = session
            .model()
            .tokenize_bytes(message.as_ref(), false, false)?;
        prompt.extend_from_slice(&session.model().tokenize_bytes(
            "<|im_end|>\n<|im_start|>assistant\n",
            false,
            true,
        )?);

        session.advance_context_with_tokens_async(prompt).await?;

        // log::info!("Generating response...");
        let resp = session.start_completing()?.into_string_async().await;
        // log::info!("Response generated: {resp}");

        Ok(resp)
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
