//! Report exporters - CSV, JSON, Markdown//!
//! This module provides different export formats for reports.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use simbank_core::Event;

/// Trait for exporting reports to different formats
pub trait ReportExporter {
    /// Export to the target format
    fn export(&self, report: &dyn ReportData) -> String;

    /// Get the file extension for this format
    fn extension(&self) -> &'static str;

    /// Get the MIME type for this format
    fn mime_type(&self) -> &'static str;
}

/// Trait for data that can be exported
pub trait ReportData {
    /// Get the report title
    fn title(&self) -> &str;

    /// Get column headers
    fn headers(&self) -> Vec<String>;

    /// Get data rows
    fn rows(&self) -> Vec<Vec<String>>;

    /// Get summary statistics as key-value pairs
    fn summary(&self) -> Vec<(String, String)>;
}

// ============================================================================
// CSV Exporter
// ============================================================================

/// CSV format exporter
pub struct CsvExporter {
    delimiter: char,
    include_header: bool,
}

impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            delimiter: ',',
            include_header: true,
        }
    }
}

impl CsvExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    pub fn without_header(mut self) -> Self {
        self.include_header = false;
        self
    }

    fn escape_csv_field(&self, field: &str) -> String {
        if field.contains(self.delimiter) || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }
}

impl ReportExporter for CsvExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let mut output = String::new();

        // Header
        if self.include_header {
            let headers: Vec<String> = report
                .headers()
                .iter()
                .map(|h| self.escape_csv_field(h))
                .collect();
            output.push_str(&headers.join(&self.delimiter.to_string()));
            output.push('\n');
        }

        // Data rows
        for row in report.rows() {
            let escaped: Vec<String> = row
                .iter()
                .map(|field| self.escape_csv_field(field))
                .collect();
            output.push_str(&escaped.join(&self.delimiter.to_string()));
            output.push('\n');
        }

        output
    }

    fn extension(&self) -> &'static str {
        "csv"
    }

    fn mime_type(&self) -> &'static str {
        "text/csv"
    }
}

// ============================================================================
// JSON Exporter
// ============================================================================

/// JSON format exporter
pub struct JsonExporter {
    pretty: bool,
}

impl Default for JsonExporter {
    fn default() -> Self {
        Self { pretty: true }
    }
}

impl JsonExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compact(mut self) -> Self {
        self.pretty = false;
        self
    }
}

impl ReportExporter for JsonExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let headers = report.headers();
        let rows = report.rows();
        let summary = report.summary();

        // Build JSON structure
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, header) in headers.iter().enumerate() {
                    let value = row.get(i).cloned().unwrap_or_default();
                    obj.insert(header.clone(), serde_json::Value::String(value));
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        let summary_obj: serde_json::Map<String, serde_json::Value> = summary
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        let output = serde_json::json!({
            "title": report.title(),
            "summary": summary_obj,
            "data": json_rows,
        });

        if self.pretty {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            serde_json::to_string(&output).unwrap_or_default()
        }
    }

    fn extension(&self) -> &'static str {
        "json"
    }

    fn mime_type(&self) -> &'static str {
        "application/json"
    }
}

// ============================================================================
// Markdown Exporter
// ============================================================================

/// Markdown format exporter
pub struct MarkdownExporter {
    include_summary: bool,
    include_toc: bool,
}

impl Default for MarkdownExporter {
    fn default() -> Self {
        Self {
            include_summary: true,
            include_toc: false,
        }
    }
}

impl MarkdownExporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn without_summary(mut self) -> Self {
        self.include_summary = false;
        self
    }

    pub fn with_toc(mut self) -> Self {
        self.include_toc = true;
        self
    }
}

impl ReportExporter for MarkdownExporter {
    fn export(&self, report: &dyn ReportData) -> String {
        let mut output = String::new();

        // Title
        output.push_str(&format!("# {}\n\n", report.title()));

        // Table of Contents
        if self.include_toc {
            output.push_str("## Table of Contents\n\n");
            if self.include_summary {
                output.push_str("- [Summary](#summary)\n");
            }
            output.push_str("- [Data](#data)\n\n");
        }

        // Summary section
        if self.include_summary {
            output.push_str("## Summary\n\n");
            for (key, value) in report.summary() {
                output.push_str(&format!("- **{}**: {}\n", key, value));
            }
            output.push('\n');
        }

        // Data table
        output.push_str("## Data\n\n");

        let headers = report.headers();
        if !headers.is_empty() {
            // Header row
            output.push_str("| ");
            output.push_str(&headers.join(" | "));
            output.push_str(" |\n");

            // Separator row
            output.push_str("| ");
            output.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
            output.push_str(" |\n");

            // Data rows
            for row in report.rows() {
                output.push_str("| ");
                output.push_str(&row.join(" | "));
                output.push_str(" |\n");
            }
        }

        output
    }

    fn extension(&self) -> &'static str {
        "md"
    }

    fn mime_type(&self) -> &'static str {
        "text/markdown"
    }
}

// ============================================================================
// Transaction Report Data
// ============================================================================

/// Transaction report data
#[derive(Debug, Clone)]
pub struct TransactionReport {
    pub title: String,
    pub transactions: Vec<TransactionRow>,
    pub total_amount: Decimal,
    pub currency: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub id: String,
    pub timestamp: String,
    pub tx_type: String,
    pub amount: String,
    pub currency: String,
    pub account_id: String,
    pub wallet_type: String,
    pub description: String,
}

impl TransactionReport {
    pub fn new(title: &str, currency: &str) -> Self {
        Self {
            title: title.to_string(),
            transactions: Vec::new(),
            total_amount: Decimal::ZERO,
            currency: currency.to_string(),
            generated_at: Utc::now(),
        }
    }

    pub fn add_transaction(&mut self, row: TransactionRow) {
        if let Ok(amount) = row.amount.parse::<Decimal>() {
            self.total_amount += amount;
        }
        self.transactions.push(row);
    }

    pub fn from_events(title: &str, events: &[Event]) -> Self {
        let mut report = Self::new(title, "");

        for event in events {
            if let Some(amount) = event.amount {
                let row = TransactionRow {
                    id: event.event_id.clone(),
                    timestamp: event.timestamp.to_rfc3339(),
                    tx_type: event.event_type.as_str().to_string(),
                    amount: amount.to_string(),
                    currency: event.currency.clone().unwrap_or_default(),
                    account_id: event.account_id.clone(),
                    wallet_type: event.to_wallet
                        .as_ref()
                        .or(event.from_wallet.as_ref())
                        .map(|w| w.as_str().to_string())
                        .unwrap_or_default(),
                    description: event.description.clone().unwrap_or_default(),
                };
                report.add_transaction(row);
            }
        }

        report
    }
}

impl ReportData for TransactionReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "ID".to_string(),
            "Timestamp".to_string(),
            "Type".to_string(),
            "Amount".to_string(),
            "Currency".to_string(),
            "Account".to_string(),
            "Wallet".to_string(),
            "Description".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.transactions
            .iter()
            .map(|t| {
                vec![
                    t.id.clone(),
                    t.timestamp.clone(),
                    t.tx_type.clone(),
                    t.amount.clone(),
                    t.currency.clone(),
                    t.account_id.clone(),
                    t.wallet_type.clone(),
                    t.description.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        vec![
            ("Total Transactions".to_string(), self.transactions.len().to_string()),
            ("Total Amount".to_string(), format!("{} {}", self.total_amount, self.currency)),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

// ============================================================================
// Account Summary Report
// ============================================================================

/// Account summary for reporting
#[derive(Debug, Clone)]
pub struct AccountSummaryReport {
    pub title: String,
    pub accounts: Vec<AccountSummaryRow>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AccountSummaryRow {
    pub account_id: String,
    pub person_name: String,
    pub person_type: String,
    pub status: String,
    pub wallet_count: usize,
    pub total_balance: String,
}

impl AccountSummaryReport {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            accounts: Vec::new(),
            generated_at: Utc::now(),
        }
    }

    pub fn add_account(&mut self, row: AccountSummaryRow) {
        self.accounts.push(row);
    }
}

impl ReportData for AccountSummaryReport {
    fn title(&self) -> &str {
        &self.title
    }

    fn headers(&self) -> Vec<String> {
        vec![
            "Account ID".to_string(),
            "Name".to_string(),
            "Type".to_string(),
            "Status".to_string(),
            "Wallets".to_string(),
            "Total Balance".to_string(),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.accounts
            .iter()
            .map(|a| {
                vec![
                    a.account_id.clone(),
                    a.person_name.clone(),
                    a.person_type.clone(),
                    a.status.clone(),
                    a.wallet_count.to_string(),
                    a.total_balance.clone(),
                ]
            })
            .collect()
    }

    fn summary(&self) -> Vec<(String, String)> {
        let active = self.accounts.iter().filter(|a| a.status == "active").count();
        vec![
            ("Total Accounts".to_string(), self.accounts.len().to_string()),
            ("Active Accounts".to_string(), active.to_string()),
            ("Generated At".to_string(), self.generated_at.to_rfc3339()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_transaction_report() -> TransactionReport {
        let mut report = TransactionReport::new("Test Transactions", "USD");
        report.add_transaction(TransactionRow {
            id: "TXN_001".to_string(),
            timestamp: "2026-01-25T10:00:00Z".to_string(),
            tx_type: "deposit".to_string(),
            amount: "100.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test deposit".to_string(),
        });
        report.add_transaction(TransactionRow {
            id: "TXN_002".to_string(),
            timestamp: "2026-01-25T11:00:00Z".to_string(),
            tx_type: "withdrawal".to_string(),
            amount: "50.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test withdrawal".to_string(),
        });
        report
    }

    #[test]
    fn test_csv_exporter() {
        let report = sample_transaction_report();
        let exporter = CsvExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("ID,Timestamp,Type,Amount"));
        assert!(output.contains("TXN_001"));
        assert!(output.contains("TXN_002"));
        assert!(output.contains("deposit"));
        assert_eq!(exporter.extension(), "csv");
    }

    #[test]
    fn test_csv_with_special_chars() {
        let mut report = TransactionReport::new("Test", "USD");
        report.add_transaction(TransactionRow {
            id: "TXN_001".to_string(),
            timestamp: "2026-01-25T10:00:00Z".to_string(),
            tx_type: "deposit".to_string(),
            amount: "100.00".to_string(),
            currency: "USD".to_string(),
            account_id: "ACC_001".to_string(),
            wallet_type: "funding".to_string(),
            description: "Test, with \"quotes\" and comma".to_string(),
        });

        let exporter = CsvExporter::new();
        let output = exporter.export(&report);

        // Should escape the description
        assert!(output.contains("\"Test, with \"\"quotes\"\" and comma\""));
    }

    #[test]
    fn test_json_exporter() {
        let report = sample_transaction_report();
        let exporter = JsonExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("\"title\": \"Test Transactions\""));
        assert!(output.contains("\"TXN_001\""));
        assert!(output.contains("\"deposit\""));
        assert_eq!(exporter.extension(), "json");
    }

    #[test]
    fn test_json_compact() {
        let report = sample_transaction_report();
        let exporter = JsonExporter::new().compact();
        let output = exporter.export(&report);

        // Compact JSON should not have newlines in the main structure
        assert!(!output.contains("  ")); // No indentation
    }

    #[test]
    fn test_markdown_exporter() {
        let report = sample_transaction_report();
        let exporter = MarkdownExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("# Test Transactions"));
        assert!(output.contains("## Summary"));
        assert!(output.contains("## Data"));
        assert!(output.contains("| ID | Timestamp | Type |"));
        assert!(output.contains("| --- | --- | --- |"));
        assert!(output.contains("| TXN_001 |"));
        assert_eq!(exporter.extension(), "md");
    }

    #[test]
    fn test_markdown_with_toc() {
        let report = sample_transaction_report();
        let exporter = MarkdownExporter::new().with_toc();
        let output = exporter.export(&report);

        assert!(output.contains("## Table of Contents"));
        assert!(output.contains("- [Summary](#summary)"));
        assert!(output.contains("- [Data](#data)"));
    }

    #[test]
    fn test_transaction_report_from_events() {
        let events = vec![
            Event::deposit("EVT_001", "CUST_001", "ACC_001", dec!(100), "USD"),
            Event::withdrawal("EVT_002", "CUST_001", "ACC_001", dec!(50), "USD"),
        ];

        let report = TransactionReport::from_events("Event Report", &events);

        assert_eq!(report.transactions.len(), 2);
        assert_eq!(report.total_amount, dec!(150));
    }

    #[test]
    fn test_account_summary_report() {
        let mut report = AccountSummaryReport::new("Account Summary");
        report.add_account(AccountSummaryRow {
            account_id: "ACC_001".to_string(),
            person_name: "Alice".to_string(),
            person_type: "customer".to_string(),
            status: "active".to_string(),
            wallet_count: 2,
            total_balance: "1000.00 USD".to_string(),
        });
        report.add_account(AccountSummaryRow {
            account_id: "ACC_002".to_string(),
            person_name: "Bob".to_string(),
            person_type: "employee".to_string(),
            status: "active".to_string(),
            wallet_count: 1,
            total_balance: "5000.00 USD".to_string(),
        });

        let exporter = MarkdownExporter::new();
        let output = exporter.export(&report);

        assert!(output.contains("ACC_001"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Total Accounts"));
    }
}