use anyhow::Result;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init_timed();

    let config = amallgam::Config {
        domain_name: hostname::get()
            .expect("Missing hostname?")
            .into_string()
            .expect("Bad UTF8 hostname?"),
    };
    let port = 3000;
    log::info!("AmaLLgaM getting ready on host {}", &config.domain_name);

    // Prepare the router
    let router = amallgam::create_router(config).await?;

    // And listen
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();

    log::info!("AmaLLgaM listening on port {port}");
    Ok(axum::serve(listener, router).await?)
}
