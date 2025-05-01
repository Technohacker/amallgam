use std::{path::PathBuf, time::Duration};

use activitypub_federation::fetch::object_id::ObjectId;
use amallgam_tester::User;
use anyhow::Result;
use axum_server::{tls_rustls::RustlsConfig, Handle};
use reqwest_middleware::reqwest::Client;
use rustls::crypto::ring;
use tokio::{signal, time};
use tracing_subscriber::EnvFilter;
use url::Url;

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

    let target_user: ObjectId<User> = get_env_with_err!("AMALLGAM_TARGET_USER")?.parse()?;
    let num_messages: usize =
        get_env_with_err!("AMALLGAM_NUM_MESSAGES")?.parse()?;
    let rate_per_sec: u32 =
        get_env_with_err!("AMALLGAM_RATE_PER_SEC")?.parse()?;

    log::info!("AmaLLgaM Tester getting ready on host {}", &domain_name);

    // Prepare the router
    let router = amallgam_tester::create_router(&domain_name, "test_user", target_user).await?;

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
    tokio::spawn(run_tester(handle.clone(), domain_name, num_messages, rate_per_sec));

    // And launch
    log::info!("AmaLLgaM listening on port {port}");
    Ok(
        axum_server::bind_rustls(([0, 0, 0, 0], port).into(), tls_config)
            .handle(handle)
            .serve(router.into_make_service())
            .await?,
    )
}

async fn run_tester(handle: Handle, domain_name: String, num_messages: usize, rate: u32) {
    let send_note_url: Url = format!("https://{domain_name}/send_note").parse().expect("Bad URL?");
    let print_stats_url: Url = format!("https://{domain_name}/print_stats").parse().expect("Bad URL?");

    let client = Client::builder().danger_accept_invalid_certs(true).build().expect("Bad client?");

    let gap = Duration::from_secs(1) / rate;
    for i in 0..num_messages {
        log::info!("Sending message {}", i + 1);
        client.post(send_note_url.clone()).send().await.expect("Bad response?");

        time::sleep(gap).await;
    }

    log::info!("Waiting before printing stats...");
    time::sleep(Duration::from_millis(5000)).await;
    client.post(print_stats_url.clone()).send().await.expect("Bad response?");

    handle.graceful_shutdown(None);
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
