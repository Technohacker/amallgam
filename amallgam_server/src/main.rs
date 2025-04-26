use std::time::Duration;

use anyhow::Result;
use axum_server::tls_rustls::RustlsConfig;
use rustls::crypto::ring;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize tracing + log bridging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    ring::default_provider()
        .install_default()
        .expect("Couldn't install crypto provider");

    // TODO: Move these to a config file
    let config = amallgam::AmallgamConfig {
        db_url: "sqlite:///amallgam/data/data.db".parse()?,
        domain_name: "amallgam.docker".to_string(),
        models_folder: "/amallgam/models".into()
    };
    let port = 443;
    log::info!("AmaLLgaM getting ready on host {}", &config.domain_name);

    // Prepare the router
    let router = amallgam::create_router(config).await?;

    // And TLS
    let tls_config = RustlsConfig::from_pem_file("ssl/cert.pem", "ssl/key.pem").await?;

    // Also the Graceful shutdown signal
    let handle = axum_server::Handle::new();
    let shutdown_signal = shutdown_signal(handle.clone());

    tokio::spawn(shutdown_signal);

    // And launch
    log::info!("AmaLLgaM listening on port {port}");
    Ok(
        axum_server::bind_rustls(([0, 0, 0, 0], port).into(), tls_config)
            .handle(handle)
            .serve(router.into_make_service())
            .await?,
    )
}

async fn shutdown_signal(handle: axum_server::Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log::info!("Received termination signal shutting down");
    handle.graceful_shutdown(Some(Duration::from_secs(10))); // 10 secs is how long docker will wait
    // to force shutdown
}
