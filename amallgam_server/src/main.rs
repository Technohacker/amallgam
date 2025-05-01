use std::{path::PathBuf, time::Duration};

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

    macro_rules! get_env_with_err {
        ($var_name:literal) => {
            std::env::var($var_name)
                .map_err(|_| anyhow::format_err!(concat!($var_name, " not found in environment")))
        };
    }

    let amallgam_root: PathBuf = get_env_with_err!("AMALLGAM_ROOT")?.parse()?;
    let domain_name = get_env_with_err!("AMALLGAM_DOMAIN")?;
    let port = get_env_with_err!("AMALLGAM_PORT")?.parse()?;
    let max_simultaneous_sessions: usize =
        get_env_with_err!("AMALLGAM_MAX_SIMULTANEOUS_SESSIONS")?.parse()?;
    let num_cores_per_session: u32 =
        get_env_with_err!("AMALLGAM_NUM_CORES_PER_SESSION")?.parse()?;

    // TODO: Move these to a config file
    let config = amallgam::AmallgamConfig {
        db_url: format!("sqlite://{}/data/data.db", amallgam_root.display()).parse()?,
        domain_name,
        models_folder: amallgam_root.join("models"),
        max_simultaneous_sessions,
        num_cores_per_session,
    };
    log::info!("AmaLLgaM getting ready on host {}", &config.domain_name);

    // Prepare the router
    let router = amallgam::create_router(config).await?;

    // And TLS
    let tls_config = RustlsConfig::from_pem_file(
        amallgam_root.join("ssl/cert.pem"),
        amallgam_root.join("ssl/key.pem"),
    )
    .await?;

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
