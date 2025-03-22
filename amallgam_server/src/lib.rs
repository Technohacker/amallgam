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
    routing::get,
};
use serde::{Deserialize, Serialize};
use users::ProtocolUser;

mod context;
mod query_builders;
mod users;

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
        .domain(config.domain_name)
        .app_data(AmallgamContext::new(&config.db_url).await?)
        .build()
        .await?;

    Ok(Router::new()
        .nest("/user", routes::user::router())
        .nest("/.well-known", routes::well_known::router())
        .layer(FederationMiddleware::new(config)))
}

mod routes {
    use super::*;

    pub mod user {
        use super::*;

        pub fn router() -> Router {
            Router::new().route("/:user_name", get(get_user_by_name))
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
    }

    pub mod well_known {
        use activitypub_federation::fetch::webfinger::{
            Webfinger, build_webfinger_response, extract_webfinger_name,
        };

        use super::*;

        pub fn router() -> Router {
            Router::new().route("/webfinger", get(webfinger))
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
#[derive(Serialize)]
struct AppError {
    #[serde(skip)]
    pub status: StatusCode,
    pub message: String,
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
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
        }
    }
}
