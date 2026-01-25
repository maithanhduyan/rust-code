//! Audit and Report commands

use anyhow::{Context, Result};
use chrono::Utc;
use simbank_core::AmlFlag;
use simbank_persistence::{EventFilter, EventReader};
use simbank_reports::{
    AmlReport, CsvExporter, JsonExporter, MarkdownExporter, ReportExporter, TransactionReport,
};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{ReportFormat, ReportType};

/// Run AML audit on events
pub async fn run_audit(
    events_dir: &Path,
    from: Option<String>,
    to: Option<String>,
    flags: Option<Vec<String>>,
    account: Option<String>,
) -> Result<()> {
    // Parse flags
    let aml_flags: Option<Vec<AmlFlag>> = flags.map(|f| {
        f.iter()
            .filter_map(|s| parse_aml_flag(s))
            .collect()
    });

    // Build filter
    let mut filter = EventFilter::new();
    if let Some(acc) = &account {
        filter = filter.account(acc);
    }
    if let Some(flags) = aml_flags {
        filter = filter.aml_flags(flags);
    }

    // Read events based on date range
    let reader = EventReader::new(events_dir);
    let events = match (&from, &to) {
        (Some(from_date), Some(to_date)) => reader.read_range(from_date, to_date)?,
        (Some(from_date), None) => {
            let today = Utc::now().format("%Y-%m-%d").to_string();
            reader.read_range(from_date, &today)?
        }
        _ => reader.read_all()?,
    };

    // Apply filter
    let events = filter.apply(events);

    println!("🔍 AML Audit Report");
    println!("   Events directory: {:?}", events_dir);
    if let Some(from) = &from {
        println!("   From: {}", from);
    }
    if let Some(to) = &to {
        println!("   To: {}", to);
    }
    if let Some(account) = &account {
        println!("   Account: {}", account);
    }
    println!();

    if events.is_empty() {
        println!("No events found matching criteria.");
        return Ok(());
    }

    // Generate AML report
    let report = AmlReport::generate("AML Audit", &events);

    println!("{}", report.summary_text());

    // Show flagged events
    let flagged = report.flagged_events_sorted();
    if !flagged.is_empty() {
        println!("\n--- Flagged Events ---");
        println!(
            "{:<12} {:<12} {:<12} {:>12} {:<8} {:<16}",
            "EVENT", "ACCOUNT", "TYPE", "AMOUNT", "CURRENCY", "FLAG"
        );
        println!("{}", "-".repeat(80));

        for event in flagged.iter().take(20) {
            println!(
                "{:<12} {:<12} {:<12} {:>12} {:<8} {:<16}",
                truncate(&event.event_id, 12),
                truncate(&event.account_id, 12),
                event.event_type,
                event.amount,
                event.currency,
                event.flag.as_str()
            );
        }

        if flagged.len() > 20 {
            println!("... and {} more flagged events", flagged.len() - 20);
        }
    }

    Ok(())
}

/// Generate a report
pub async fn generate_report(
    events_dir: &Path,
    format: ReportFormat,
    output: Option<PathBuf>,
    report_type: ReportType,
) -> Result<()> {
    // Read all events
    let reader = EventReader::new(events_dir);
    let events = reader.read_all()?;

    if events.is_empty() {
        println!("No events found. Nothing to report.");
        return Ok(());
    }

    // Generate report content
    let content = match report_type {
        ReportType::Aml => {
            let report = AmlReport::generate("AML Compliance Report", &events);
            export_report(&report, format)
        }
        ReportType::Transactions => {
            let report = TransactionReport::from_events("Transaction Report", &events);
            export_report(&report, format)
        }
        ReportType::Accounts => {
            // For accounts, we generate a simple transaction report grouped by account
            let report = TransactionReport::from_events("Account Activity Report", &events);
            export_report(&report, format)
        }
    };

    // Output
    match output {
        Some(path) => {
            fs::write(&path, &content).context("Failed to write report file")?;
            println!("✅ Report generated: {:?}", path);
        }
        None => {
            println!("{}", content);
        }
    }

    Ok(())
}

/// Export report to specified format
fn export_report(report: &dyn simbank_reports::ReportData, format: ReportFormat) -> String {
    match format {
        ReportFormat::Csv => CsvExporter::new().export(report),
        ReportFormat::Json => JsonExporter::new().export(report),
        ReportFormat::Markdown => MarkdownExporter::new().with_toc().export(report),
    }
}

/// Parse AML flag string
fn parse_aml_flag(s: &str) -> Option<AmlFlag> {
    match s.to_lowercase().as_str() {
        "large_amount" => Some(AmlFlag::LargeAmount),
        "near_threshold" => Some(AmlFlag::NearThreshold),
        "unusual_pattern" => Some(AmlFlag::UnusualPattern),
        "cross_border" => Some(AmlFlag::CrossBorder),
        "high_risk_country" => Some(AmlFlag::HighRiskCountry),
        "new_account_large_tx" => Some(AmlFlag::NewAccountLargeTx),
        "rapid_withdrawal" => Some(AmlFlag::RapidWithdrawal),
        _ => None,
    }
}

/// Truncate string for display
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
