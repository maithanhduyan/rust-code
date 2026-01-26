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
    Audit {
        /// Also verify digital signatures
        #[arg(long)]
        verify_signatures: bool,
    },

    // === Phase 2: Trade and Fee ===

    /// Execute a trade between two users
    Trade {
        /// Maker user ID (seller)
        maker: String,
        /// Taker user ID (buyer)
        taker: String,
        /// Amount to sell
        #[arg(long)]
        sell: Decimal,
        /// Asset to sell
        #[arg(long)]
        sell_asset: String,
        /// Amount to buy
        #[arg(long)]
        buy: Decimal,
        /// Asset to buy
        #[arg(long)]
        buy_asset: String,
        /// Optional fee amount (charged to maker)
        #[arg(long)]
        fee: Option<Decimal>,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Charge a fee from a user
    Fee {
        /// User ID
        user: String,
        /// Fee amount
        amount: Decimal,
        /// Asset/currency code
        asset: String,
        /// Fee type (trading, withdrawal, etc.)
        #[arg(long, default_value = "trading")]
        fee_type: String,
        /// Optional correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Generate a new system key
    Keygen {
        /// Output file path
        #[arg(long, default_value = "system.key")]
        output: PathBuf,
    },

    // === Phase 2.1: Trade History ===

    /// List trade history
    Trades {
        /// Filter by user ID
        #[arg(long)]
        user: Option<String>,
        /// Filter by base asset (requires --quote)
        #[arg(long)]
        base: Option<String>,
        /// Filter by quote asset (requires --base)
        #[arg(long)]
        quote: Option<String>,
        /// Maximum number of trades to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },
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
            let projection_path = ctx.projection_path().to_path_buf();
            let data_path = cli.data.clone();

            // Drop existing context to release SQLite connection
            drop(ctx);

            if reset {
                println!("🗑️  Dropping projections...");
                if projection_path.exists() {
                    std::fs::remove_file(&projection_path)?;
                    println!("   Deleted {}", projection_path.display());
                }
            }

            // Recreate context to replay
            let new_ctx = AppContext::new(&data_path).await?;
            println!(
                "✅ Replayed {} entries",
                new_ctx.last_sequence()
            );
        }

        Commands::Audit { verify_signatures } => {
            use bibank_events::EventReader;
            use bibank_ledger::hash::verify_chain;

            let reader = EventReader::from_directory(ctx.journal_path())?;
            let entries = reader.read_all()?;

            // Verify hash chain
            match verify_chain(&entries) {
                Ok(()) => {
                    println!("✅ Hash chain verified ({} entries)", entries.len());
                }
                Err(e) => {
                    println!("❌ Hash chain broken: {}", e);
                    return Ok(());
                }
            }

            // Verify signatures if requested
            if verify_signatures {
                let mut signed_count = 0;
                let mut unsigned_count = 0;

                for entry in &entries {
                    if entry.signatures.is_empty() {
                        unsigned_count += 1;
                    } else {
                        match entry.verify_signatures() {
                            Ok(()) => signed_count += 1,
                            Err(e) => {
                                println!("❌ Signature verification failed at seq {}: {}", entry.sequence, e);
                                return Ok(());
                            }
                        }
                    }
                }

                println!("✅ Signatures verified: {} signed, {} unsigned (Phase 1)", signed_count, unsigned_count);
            }
        }

        Commands::Trade {
            maker,
            taker,
            sell,
            sell_asset,
            buy,
            buy_asset,
            fee,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());

            if let Some(fee_amount) = fee {
                commands::trade_with_fee(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    fee_amount,
                    &correlation_id,
                ).await?;
            } else {
                commands::trade(
                    &mut ctx, &maker, &taker,
                    sell, &sell_asset,
                    buy, &buy_asset,
                    &correlation_id,
                ).await?;
            }
        }

        Commands::Fee {
            user,
            amount,
            asset,
            fee_type,
            correlation_id,
        } => {
            let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            commands::fee(&mut ctx, &user, amount, &asset, &fee_type, &correlation_id).await?;
        }

        Commands::Keygen { output } => {
            use bibank_ledger::{Signer, SystemSigner};

            let signer = SystemSigner::generate();
            let seed = signer.seed_hex();
            let pubkey = signer.public_key_hex();

            std::fs::write(&output, &seed)?;
            println!("✅ Generated system key");
            println!("   Private key saved to: {}", output.display());
            println!("   Public key: {}", pubkey);
            println!("");
            println!("To use: export BIBANK_SYSTEM_KEY={}", seed);
        }

        Commands::Trades {
            user,
            base,
            quote,
            limit,
        } => {
            let pair = match (&base, &quote) {
                (Some(b), Some(q)) => Some((b.as_str(), q.as_str())),
                _ => None,
            };
            commands::trades(&ctx, user.as_deref(), pair, limit).await?;
        }
    }

    Ok(())
}
