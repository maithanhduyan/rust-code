//! # Simbank Reports
//!
//! Report generation - CSV, JSON, Markdown, AML reports.
//!
//! This crate provides export functionality for different report formats
//! and AML compliance reporting suitable for regulatory audits.
//!
//! ## Exporters
//!
//! - [`CsvExporter`] - CSV format with proper escaping
//! - [`JsonExporter`] - JSON format (pretty or compact)
//! - [`MarkdownExporter`] - Markdown tables for documentation
//!
//! ## Reports
//!
//! - [`TransactionReport`] - Transaction history reports
//! - [`AccountSummaryReport`] - Account overview reports
//! - [`AmlReport`] - AML compliance reports with risk scoring
//! - [`VelocityReport`] - Transaction velocity analysis
//!
//! ## Example
//!
//! ```rust,ignore
//! use simbank_reports::{CsvExporter, MarkdownExporter, ReportExporter, TransactionReport};
//!
//! let report = TransactionReport::new("Monthly Report", "USD");
//! let csv_exporter = CsvExporter::new();
//! let csv_output = csv_exporter.export(&report);
//!
//! let md_exporter = MarkdownExporter::new().with_toc();
//! let md_output = md_exporter.export(&report);
//! ```

pub mod exporters;
pub mod aml_report;

// Re-export main types
pub use exporters::{
    ReportExporter,
    ReportData,
    CsvExporter,
    JsonExporter,
    MarkdownExporter,
    TransactionReport,
    TransactionRow,
    AccountSummaryReport,
    AccountSummaryRow,
};

pub use aml_report::{
    AmlReport,
    FlaggedEvent,
    RiskLevel,
    VelocityReport,
    VelocityAnalysis,
};
