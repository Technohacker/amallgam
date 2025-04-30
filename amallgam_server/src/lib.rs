use std::time::Duration;

use activitypub_federation::{
    axum::json::FederationJson,
    config::{Data, FederationConfig, FederationMiddleware},
    protocol::context::WithContext,
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
use tower_http::trace::TraceLayer;

mod activities;
mod context;
mod objects;

pub use self::context::AmallgamConfig;
use self::context::{AmallgamContext, ArcAmallgamContext};

type Result<T> = std::result::Result<T, AppError>;

pub async fn create_router(config: AmallgamConfig) -> anyhow::Result<Router> {
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
        .domain(&config.domain_name)
        .app_data(AmallgamContext::new(config).await?)
        .build()
        .await?;

    let router = Router::new().route(
        "/_internal/run_pending_notes",
        post(routes::run_pending_notes),
    );
    let router = routes::admin::configure_router(router);
    let router = routes::user::configure_router(router);
    let router = routes::well_known::configure_router(router);

    Ok(router
        .layer(TraceLayer::new_for_http())
        .layer(FederationMiddleware::new(fed_config)))
}

mod routes {
    use activitypub_federation::activity_sending::SendActivityTask;

    use super::*;

    #[axum::debug_handler]
    pub(super) async fn run_pending_notes(ctx: Data<ArcAmallgamContext>) -> Result<()> {
        while let Ok(pending) = ctx.pending_notes.try_write()?.try_recv() {
            let bot_user = pending
                .activity
                .actor
                .dereference_local(&ctx)
                .await
                .expect("Missing bot user?");

            let msg = WithContext::new_default(pending.activity);

            let sends =
                SendActivityTask::prepare(&msg, &bot_user, pending.target_inboxes.clone(), &ctx)
                    .await?;

            for send in sends {
                send.sign_and_send(&ctx).await?;
            }
        }

        Ok(())
    }

    pub mod admin {
        use axum::routing::put;

        use crate::context::{ModelConfig, ModelId};

        use super::*;

        pub fn configure_router(router: Router) -> Router {
            router
                .route("/admin/upsert_model_config", put(upsert_model_config))
                .route("/admin/upsert_bot_config", put(upsert_bot_config))
                .route("/admin/add_bot_alias", put(add_bot_alias))
        }

        #[axum::debug_handler]
        async fn upsert_model_config(
            ctx: Data<ArcAmallgamContext>,
            Json(model_config): Json<ModelConfig>,
        ) -> Result<()> {
            Ok(ctx.upsert_model_config(model_config).await?)
        }

        #[derive(Deserialize)]
        struct BotConfig {
            user_id: String,
            model_id: ModelId,
            system_prompt: String,
        }

        #[axum::debug_handler]
        async fn upsert_bot_config(
            ctx: Data<ArcAmallgamContext>,
            Json(bot_config): Json<BotConfig>,
        ) -> Result<()> {
            Ok(ctx
                .upsert_user(ctx.new_bot_user(
                    &bot_config.user_id,
                    bot_config.model_id,
                    &bot_config.system_prompt,
                ))
                .await?)
        }

        #[derive(Deserialize)]
        struct BotAlias {
            user_id: String,
            alias_id: String,
        }

        #[axum::debug_handler]
        async fn add_bot_alias(
            ctx: Data<ArcAmallgamContext>,
            Json(bot_alias): Json<BotAlias>,
        ) -> Result<()> {
            Ok(ctx
                .add_bot_alias(&bot_alias.user_id, &bot_alias.alias_id)
                .await?)
        }
    }

    pub mod user {
        use activitypub_federation::{axum::inbox::ActivityData, axum::inbox::receive_activity};

        use crate::objects::users::{ProtocolUser, User, UserAllowedActivities};

        use super::*;

        pub fn configure_router(router: Router) -> Router {
            router
                .route("/user/:user_id/", get(get_user_by_userid))
                .route("/user/:user_id/inbox", post(handle_user_inbox))
        }

        #[axum::debug_handler]
        async fn get_user_by_userid(
            Path(user_id): Path<String>,
            ctx: Data<ArcAmallgamContext>,
        ) -> Result<FederationJson<WithContext<ProtocolUser>>> {
            let user = ctx
                .get_bot_user_by_userid(&user_id)
                .await?
                .ok_or_else(|| anyhow::format_err!("User not found"))?;

            let user = user.into_json(&ctx).await?;

            Ok(FederationJson(WithContext::new_default(user)))
        }

        #[axum::debug_handler]
        async fn handle_user_inbox(
            ctx: Data<ArcAmallgamContext>,
            activity: ActivityData,
        ) -> Result<()> {
            Ok(receive_activity::<UserAllowedActivities, User, _>(activity, &ctx).await?)
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
            ctx: Data<ArcAmallgamContext>,
        ) -> Result<Json<Webfinger>> {
            log::info!("WebFinger for {}", query.resource);
            let user_id = extract_webfinger_name(&query.resource, &ctx)?;

            let db_user = ctx
                .get_bot_user_by_userid(user_id)
                .await?
                .ok_or_else(|| anyhow::format_err!("User not found"))?;

            Ok(Json(build_webfinger_response(query.resource, db_user.id())))
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
