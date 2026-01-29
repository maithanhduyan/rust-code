//! CLI Application

mod commands;

use clap::Parser;
use commands::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::User(args) => commands::user::handle(args)?,
        Commands::Item(args) => commands::item::handle(args)?,
        Commands::Config => commands::config::handle()?,
    }

    Ok(())
}
