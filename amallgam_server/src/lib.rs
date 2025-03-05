use activitypub_federation::config::{FederationConfig, FederationMiddleware};
use anyhow::Result;
use axum::{Router, extract::Path, routing::get};

mod bot_user;
mod context;

use self::context::AmallgamContext;

pub struct Config {
    /// Domain name to listen for federation requests on
    pub domain_name: String,
}

pub async fn create_router(config: Config) -> Result<Router> {
    let config = FederationConfig::builder()
        .domain(config.domain_name)
        .app_data(AmallgamContext::new())
        .build()
        .await?;

    Ok(Router::new()
        .route(
            "/user/:user_id",
            get(async |Path(user_id): Path<String>| format!("Hello, {user_id}!")),
        )
        .layer(FederationMiddleware::new(config)))
}
