//! CLI Commands

pub mod config;
pub mod item;
pub mod user;

use clap::{Parser, Subcommand};

/// Rust Workspace CLI Application
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage users
    User(user::UserArgs),
    
    /// Manage items
    Item(item::ItemArgs),
    
    /// Show configuration
    Config,
}
