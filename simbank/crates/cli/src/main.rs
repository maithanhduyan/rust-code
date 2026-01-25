//! Simbank CLI - Banking operations from command line
//!
//! Usage:
//! ```bash
//! simbank account create --type customer --name "Alice"
//! simbank deposit ACC_001 100 USDT --to funding
//! simbank transfer ACC_001 50 USDT --from funding --to spot
//! simbank withdraw ACC_001 30 USDT --from funding
//! simbank audit --from 2026-01-01 --flags large_amount
//! simbank report --format markdown --output report.md
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;
use std::path::PathBuf;

mod commands;
mod db;

use commands::{account, audit, wallet};

/// Simbank - A banking DSL demonstration with SQLite + Event Sourcing
#[derive(Parser)]
#[command(name = "simbank")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Database file path
    #[arg(long, default_value = "data/simbank.db", global = true)]
    pub db: PathBuf,

    /// Events directory path
    #[arg(long, default_value = "data/events", global = true)]
    pub events_dir: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Account management
    Account {
        #[command(subcommand)]
        action: AccountAction,
    },

    /// Deposit funds to an account
    Deposit {
        /// Account ID (e.g., ACC_001)
        account_id: String,
        /// Amount to deposit
        amount: Decimal,
        /// Currency code (e.g., USD, USDT, BTC)
        currency: String,
        /// Target wallet type
        #[arg(long, default_value = "funding")]
        to: WalletTypeArg,
    },

    /// Withdraw funds from an account
    Withdraw {
        /// Account ID
        account_id: String,
        /// Amount to withdraw
        amount: Decimal,
        /// Currency code
        currency: String,
        /// Source wallet type
        #[arg(long, default_value = "funding")]
        from: WalletTypeArg,
    },

    /// Transfer funds between wallets (internal)
    Transfer {
        /// Account ID
        account_id: String,
        /// Amount to transfer
        amount: Decimal,
        /// Currency code
        currency: String,
        /// Source wallet type
        #[arg(long)]
        from: WalletTypeArg,
        /// Destination wallet type
        #[arg(long)]
        to: WalletTypeArg,
    },

    /// Audit transactions for AML compliance
    Audit {
        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,
        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
        /// AML flags to filter (comma-separated)
        #[arg(long, value_delimiter = ',')]
        flags: Option<Vec<String>>,
        /// Account ID to audit (optional)
        #[arg(long)]
        account: Option<String>,
    },

    /// Generate reports
    Report {
        /// Report format
        #[arg(long, default_value = "markdown")]
        format: ReportFormat,
        /// Output file path
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Report type
        #[arg(long, default_value = "aml")]
        report_type: ReportType,
    },

    /// Initialize database with schema and seed data
    Init {
        /// Force re-initialization (drops existing data)
        #[arg(long)]
        force: bool,
    },

    /// Show database status
    Status,
}

#[derive(Subcommand)]
pub enum AccountAction {
    /// Create a new account
    Create {
        /// Person type
        #[arg(long, short = 't')]
        r#type: PersonTypeArg,
        /// Person name
        #[arg(long, short)]
        name: String,
        /// Email (optional)
        #[arg(long, short)]
        email: Option<String>,
    },
    /// List all accounts
    List {
        /// Filter by person type
        #[arg(long, short = 't')]
        r#type: Option<PersonTypeArg>,
    },
    /// Show account details
    Show {
        /// Account ID
        account_id: String,
    },
    /// Show account balances
    Balance {
        /// Account ID
        account_id: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum PersonTypeArg {
    Customer,
    Employee,
    Shareholder,
    Manager,
    Auditor,
}

impl PersonTypeArg {
    pub fn to_core_type(&self) -> simbank_core::PersonType {
        match self {
            PersonTypeArg::Customer => simbank_core::PersonType::Customer,
            PersonTypeArg::Employee => simbank_core::PersonType::Employee,
            PersonTypeArg::Shareholder => simbank_core::PersonType::Shareholder,
            PersonTypeArg::Manager => simbank_core::PersonType::Manager,
            PersonTypeArg::Auditor => simbank_core::PersonType::Auditor,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum WalletTypeArg {
    Spot,
    Funding,
    Margin,
    Futures,
    Earn,
}

impl WalletTypeArg {
    pub fn to_core_type(&self) -> simbank_core::WalletType {
        match self {
            WalletTypeArg::Spot => simbank_core::WalletType::Spot,
            WalletTypeArg::Funding => simbank_core::WalletType::Funding,
            WalletTypeArg::Margin => simbank_core::WalletType::Margin,
            WalletTypeArg::Futures => simbank_core::WalletType::Futures,
            WalletTypeArg::Earn => simbank_core::WalletType::Earn,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ReportFormat {
    Csv,
    Json,
    Markdown,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ReportType {
    Aml,
    Transactions,
    Accounts,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Ensure data directories exist
    if let Some(parent) = cli.db.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::create_dir_all(&cli.events_dir).ok();

    match cli.command {
        Commands::Init { force } => {
            db::init_database(&cli.db, force).await?;
            println!("✅ Database initialized at {:?}", cli.db);
        }

        Commands::Status => {
            db::show_status(&cli.db).await?;
        }

        Commands::Account { action } => {
            account::handle(&cli.db, &cli.events_dir, action).await?;
        }

        Commands::Deposit {
            account_id,
            amount,
            currency,
            to,
        } => {
            wallet::deposit(&cli.db, &cli.events_dir, &account_id, amount, &currency, to).await?;
        }

        Commands::Withdraw {
            account_id,
            amount,
            currency,
            from,
        } => {
            wallet::withdraw(&cli.db, &cli.events_dir, &account_id, amount, &currency, from).await?;
        }

        Commands::Transfer {
            account_id,
            amount,
            currency,
            from,
            to,
        } => {
            wallet::transfer(&cli.db, &cli.events_dir, &account_id, amount, &currency, from, to).await?;
        }

        Commands::Audit {
            from,
            to,
            flags,
            account,
        } => {
            audit::run_audit(&cli.events_dir, from, to, flags, account).await?;
        }

        Commands::Report {
            format,
            output,
            report_type,
        } => {
            audit::generate_report(&cli.events_dir, format, output, report_type).await?;
        }
    }

    Ok(())
}
