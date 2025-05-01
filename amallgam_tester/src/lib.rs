use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
};

use activitypub_federation::{
    axum::json::FederationJson,
    config::{Data, FederationConfig, FederationMiddleware},
    fetch::object_id::ObjectId,
    http_signatures::generate_actor_keypair,
    protocol::{context::WithContext, public_key::PublicKey},
    traits::Object,
};

use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use reqwest_middleware::reqwest::{Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, time::Instant};
use tower_http::trace::TraceLayer;
use url::Url;

mod create;
mod note;
mod tester_core;
mod user;

pub use self::user::User;

type Result<T> = std::result::Result<T, AppError>;

pub struct AmallgamTesterContext {
    pub base_url: Url,
    pub user: User,

    pub target_user: ObjectId<User>,
    pub failed_notes: AtomicUsize,
    pub note_starts: RwLock<HashMap<Url, Instant>>,
    pub note_ends: RwLock<HashMap<Url, Instant>>,
}

impl AmallgamTesterContext {
    pub fn new(domain_name: &str, test_user: &str, target_user: ObjectId<User>) -> Arc<Self> {
        let base_url: Url = format!("https://{domain_name}/").parse().expect("Bad URL?");

        let user_url: Url = base_url
            .join(&format!("./user/{test_user}"))
            .expect("Bad URL?");
        let kp = generate_actor_keypair().expect("Bad Keypair?");

        let user = User {
            id: ObjectId::from(user_url.clone()),
            kind: activitypub_federation::kinds::actor::PersonType::Person,
            preferred_username: test_user.to_string(),
            name: test_user.to_string(),
            inbox: user_url.join("./inbox").expect("Bad URL?"),
            outbox: user_url.join("./outbox").expect("Bad URL?"),
            public_key: PublicKey {
                id: format!("{user_url}#main-key"),
                owner: user_url,
                public_key_pem: kp.public_key,
            },
            private_key: Some(kp.private_key),
        };

        Arc::new(Self {
            base_url,
            user,

            target_user,
            failed_notes: AtomicUsize::new(0),
            note_starts: RwLock::new(HashMap::new()),
            note_ends: RwLock::new(HashMap::new()),
        })
    }
}

pub async fn create_router(
    domain_name: &str,
    test_user: &str,
    target_user: ObjectId<User>,
) -> anyhow::Result<Router> {
    let timeout = Duration::from_secs(10);
    let http_client = Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(Policy::none())
        .timeout(timeout)
        .connect_timeout(timeout)
        .build()
        .expect("Couldn't construct reqwest Client?");

    let fed_config = FederationConfig::builder()
        .client(http_client.into())
        .debug(true)
        .domain(domain_name)
        .app_data(AmallgamTesterContext::new(
            domain_name,
            test_user,
            target_user,
        ))
        .build()
        .await?;

    let router = Router::new()
        .route("/send_note", post(routes::send_note))
        .route("/print_stats", post(routes::print_stats));
    let router = routes::user::configure_router(router);
    let router = routes::well_known::configure_router(router);

    Ok(router
        .layer(TraceLayer::new_for_http())
        .layer(FederationMiddleware::new(fed_config)))
}

mod routes {
    use activitypub_federation::{activity_sending::SendActivityTask, kinds::public};

    use crate::note::Mention;

    use super::*;

    #[axum::debug_handler]
    pub(super) async fn send_note(ctx: Data<Arc<AmallgamTesterContext>>) -> Result<()> {
        let note = ctx.new_create_activity(
            ctx.target_user.clone(),
            vec![public()],
            vec![],
            ctx.new_note(
                ctx.user.id.clone(),
                vec![ctx.target_user.inner().clone()],
                vec![],
                "",
                None,
                vec![Mention::for_user(ctx.target_user.clone())],
            ),
        );

        ctx.note_starts
            .write()
            .await
            .insert(note.object.id.inner().clone(), Instant::now());

        let msg = WithContext::new_default(note);

        let sends = SendActivityTask::prepare(&msg, &ctx.user, vec![], &ctx).await?;

        let res = async {
            for send in sends {
                send.sign_and_send(&ctx).await?;
            }

            anyhow::Ok(())
        };

        if res.await.is_err() {
            log::warn!("Note failed");
            ctx.failed_notes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        Ok(())
    }

    #[axum::debug_handler]
    pub(super) async fn print_stats(ctx: Data<Arc<AmallgamTesterContext>>) {
        log::info!("===================== Run Done ====================");
        let mut successful: u128 = 0;
        let mut not_replied: u128 = 0;

        let mut durations = vec![];

        let note_ends = ctx.note_ends.blocking_read();
        for (key, start) in ctx.note_starts.blocking_read().iter() {
            let end = note_ends.get(key);

            if let Some(end) = end {
                durations.push(end.duration_since(*start));
                successful += 1;
            } else {
                not_replied += 1;
            }
        }

        let sum_dur: Duration = durations.into_iter().sum();
        let avg_dur = sum_dur.as_millis() / successful;

        log::info!("\tSuccessful Replies:    {successful}");
        log::info!("\tReplies not received:  {not_replied}");
        log::info!("\tAverage time to reply: {avg_dur} seconds");

        log::info!("");
        log::info!("===================================================");
    }

    pub mod user {
        use activitypub_federation::{axum::inbox::ActivityData, axum::inbox::receive_activity};

        use crate::{create::Create, note::Note, user::User};

        use super::*;

        pub fn configure_router(router: Router) -> Router {
            router
                .route("/user/:user_id/", get(get_user_by_userid))
                .route("/user/:user_id/inbox", post(handle_user_inbox))
        }

        #[axum::debug_handler]
        async fn get_user_by_userid(
            Path(user_id): Path<String>,
            ctx: Data<Arc<AmallgamTesterContext>>,
        ) -> Result<FederationJson<WithContext<User>>> {
            if user_id == ctx.user.name {
                Ok(FederationJson(WithContext::new_default(
                    ctx.user.clone().into_json(&ctx).await?,
                )))
            } else {
                Err(anyhow::format_err!("User not found").into())
            }
        }

        #[axum::debug_handler]
        async fn handle_user_inbox(
            ctx: Data<Arc<AmallgamTesterContext>>,
            activity: ActivityData,
        ) -> Result<()> {
            Ok(receive_activity::<Create<Note>, User, _>(activity, &ctx).await?)
        }
    }

    pub mod well_known {
        use activitypub_federation::{
            fetch::webfinger::{Webfinger, build_webfinger_response, extract_webfinger_name},
            traits::Actor,
        };

        use super::*;

        pub fn configure_router(router: Router) -> Router {
            router.route("/.well-known/webfinger", get(webfinger))
        }

        #[derive(Deserialize)]
        struct WebfingerQuery {
            resource: String,
        }

        async fn webfinger(
            Query(query): Query<WebfingerQuery>,
            ctx: Data<Arc<AmallgamTesterContext>>,
        ) -> Result<Json<Webfinger>> {
            log::info!("WebFinger for {}", query.resource);
            let user_id = extract_webfinger_name(&query.resource, &ctx)?;

            if user_id == ctx.user.name {
                Ok(Json(build_webfinger_response(
                    query.resource,
                    ctx.user.id(),
                )))
            } else {
                Err(anyhow::format_err!("User not found").into())
            }
        }
    }
}

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug, Serialize)]
struct AppError {
    #[serde(skip)]
    pub status: StatusCode,
    #[serde(skip)]
    pub error: anyhow::Error,
    pub message: String,
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        log::error!("Error! {:?}", &self.error);
        (self.status, Json(self)).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err = err.into();
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{err}"),
            error: err,
        }
    }
}
