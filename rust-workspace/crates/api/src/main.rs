//! API Server Application

mod handlers;
mod routes;
mod state;

use anyhow::Result;
use std::net::SocketAddr;
use utils::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let config = AppConfig::from_env();
    let app = routes::create_router();
    
    let addr: SocketAddr = config.bind_address().parse()?;
    log::info!("🚀 API server starting at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
