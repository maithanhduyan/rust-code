//! BiBank CLI - Main entry point

use bibank_rpc::{commands, AppContext};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "bibank")]
#[command(about = "BiBank - Financial State OS", long_about = None)]
struct Cli {
    /// Data directory path
    #[arg(short, long, default_value = "./data")]
    data: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the system with Genesis entry
    Init,

    /// Deposit funds to a user
    Deposit {
        /// User ID (will be uppercased)
        user: String,
        /// Amount to deposit
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Transfer funds between users
    Transfer {
        /// Source user ID
        from: String,
        /// Destination user ID
        to: String,
        /// Amount to transfer
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Withdraw funds from a user
    Withdraw {
        /// User ID
        user: String,
        /// Amount to withdraw
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Check balance for a user
    Balance {
        /// User ID
        user: String,
    },

    /// Replay events (rebuild projections)
    Replay {
        /// Drop projections before replay
        #[arg(long)]
        reset: bool,
    },

    /// Audit the ledger (verify hash chain)
    Audit,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Create application context
    let mut ctx = AppContext::new(&cli.data).await?;

    match cli.command {
        Commands::Init => {
            let correlation_id = Uuid::new_v4().to_string();
            commands::init(&mut ctx, &correlation_id).await?;
        }

        Commands::Deposit {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::deposit(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Transfer {
            from,
            to,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::transfer(&mut ctx, &from, &to, amount, &asset, &correlation_id).await?;
        }

        Commands::Withdraw {
            user,
            amount,
            asset,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::withdraw(&mut ctx, &user, amount, &asset, &correlation_id).await?;
        }

        Commands::Balance { user } => {
            commands::balance(&ctx, &user).await?;
        }

        Commands::Replay { reset } => {
            if reset {
                println!("🗑️  Dropping projections...");
                if ctx.projection_path().exists() {
                    std::fs::remove_file(ctx.projection_path())?;
                }
            }

            // Recreate context to replay
            let ctx = AppContext::new(&cli.data).await?;
            println!(
                "✅ Replayed {} entries",
                ctx.last_sequence()
            );
        }

        Commands::Audit => {
            use bibank_events::EventReader;
            use bibank_ledger::hash::verify_chain;

            let reader = EventReader::from_directory(ctx.journal_path())?;
            let entries = reader.read_all()?;

            match verify_chain(&entries) {
                Ok(()) => {
                    println!("✅ Hash chain verified ({} entries)", entries.len());
                }
                Err(e) => {
                    println!("❌ Hash chain broken: {}", e);
                }
            }
        }
    }

    Ok(())
}
