//! Apex - High-performance reverse proxy
//!
//! # Usage
//! ```bash
//! apex --config apex.toml
//! apex --config apex.toml --http2    # Use HTTP/2 for higher throughput
//! apex --config apex.toml --ultra    # Ultra mode (max performance, single backend)
//! apex --config apex.toml --check    # Validate config only
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use apex_config::ConfigLoader;
use apex_server::{ProxyHandler, Http2Handler};

/// Apex - High-performance reverse proxy written in Rust
#[derive(Parser, Debug)]
#[command(name = "apex")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "apex.toml")]
    config: PathBuf,

    /// Validate configuration and exit
    #[arg(long)]
    check: bool,

    /// Use HTTP/2 for client and backend (higher throughput)
    #[arg(long)]
    http2: bool,

    /// Ultra mode - maximum performance (single backend, GET optimized)
    #[arg(long)]
    ultra: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(&args.log_level);

    tracing::info!("Apex v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let loader = ConfigLoader::load_file(&args.config)
        .with_context(|| format!("Failed to load config from {:?}", args.config))?;

    let config = loader.get();

    tracing::info!("Loaded configuration with {} routes", config.routes.len());

    // Config check mode
    if args.check {
        tracing::info!("Configuration is valid");
        return Ok(());
    }

    // Create and run server
    if args.ultra {
        tracing::info!("Starting Apex ULTRA proxy server (maximum throughput)...");
        let handler = Http2Handler::from_config_ultra(&config);
        handler.run().await?;
    } else if args.http2 {
        tracing::info!("Starting Apex HTTP/2 proxy server (high-throughput mode)...");
        let handler = Http2Handler::from_config(&config);
        handler.run().await?;
    } else {
        tracing::info!("Starting Apex proxy server...");
        let handler = ProxyHandler::from_config(&config);
        handler.run().await?;
    }

    Ok(())
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();
}
