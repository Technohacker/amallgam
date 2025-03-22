use anyhow::Result;
use axum_server::tls_rustls::RustlsConfig;
use rustls::crypto::ring;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init_timed();
    ring::default_provider()
        .install_default()
        .expect("Couldn't install crypto provider");

    // TODO: Move these to a config file
    let config = amallgam::Config {
        db_url: "sqlite::memory:".to_string(),
        domain_name: "amallgam.docker".to_string(),
    };
    let port = 443;
    log::info!("AmaLLgaM getting ready on host {}", &config.domain_name);

    // Prepare the router
    let router = amallgam::create_router(config).await?;

    // And listen
    let tls_config = RustlsConfig::from_pem_file("ssl/cert.pem", "ssl/key.pem").await?;

    log::info!("AmaLLgaM listening on port {port}");
    Ok(
        axum_server::bind_rustls(([0, 0, 0, 0], port).into(), tls_config)
            .serve(router.into_make_service())
            .await?,
    )
}
