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
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

mod activities;
mod context;
mod objects;

use self::context::AmallgamContext;

type Result<T> = std::result::Result<T, AppError>;

pub struct Config {
    /// Domain name to listen for federation requests on
    pub domain_name: String,

    /// URL to database
    pub db_url: String,
}

pub async fn create_router(config: Config) -> anyhow::Result<Router> {
    let config = FederationConfig::builder()
        .debug(true)
        .domain(config.domain_name)
        .app_data(AmallgamContext::new(&config.db_url).await?)
        .build()
        .await?;

    let router = Router::new();
    let router = routes::user::configure_router(router);
    let router = routes::well_known::configure_router(router);

    Ok(router
        .layer(TraceLayer::new_for_http())
        .layer(FederationMiddleware::new(config)))
}

mod routes {
    use super::*;

    pub mod user {
        use activitypub_federation::{axum::inbox::ActivityData, axum::inbox::receive_activity};

        use crate::objects::users::{ProtocolUser, User, UserAllowedActivities};

        use super::*;

        pub fn configure_router(router: Router) -> Router {
            router
                .route("/user/:user_name", get(get_user_by_name))
                .route("/user/:user_name/inbox", post(handle_user_inbox))
        }

        #[axum::debug_handler]
        async fn get_user_by_name(
            Path(user_name): Path<String>,
            ctx: Data<AmallgamContext>,
        ) -> Result<FederationJson<WithContext<ProtocolUser>>> {
            let user = ctx
                .app_data()
                .get_user_by_username(&user_name)
                .await?
                .ok_or_else(|| anyhow::format_err!("User not found"))?;

            let user = user.into_json(&ctx).await?;

            Ok(FederationJson(WithContext::new_default(user)))
        }

        #[axum::debug_handler]
        async fn handle_user_inbox(
            ctx: Data<AmallgamContext>,
            activity: ActivityData,
        ) -> Result<()> {
            Ok(receive_activity::<UserAllowedActivities, User, _>(activity, &ctx).await?)
        }
    }

    pub mod well_known {
        use activitypub_federation::fetch::webfinger::{
            Webfinger, build_webfinger_response, extract_webfinger_name,
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
            ctx: Data<AmallgamContext>,
        ) -> Result<Json<Webfinger>> {
            log::info!("WebFinger for {}", query.resource);
            let name = extract_webfinger_name(&query.resource, &ctx)?;

            let db_user = ctx
                .get_user_by_username(name)
                .await?
                .ok_or_else(|| anyhow::format_err!("User not found"))?;

            Ok(Json(build_webfinger_response(
                query.resource,
                db_user.id.into_inner(),
            )))
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
