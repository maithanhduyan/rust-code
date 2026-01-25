//! # Example 04: Auditor AML Scan
//!
//! This example demonstrates AML (Anti-Money Laundering) auditing:
//! 1. Auditor scans transactions for suspicious patterns
//! 2. Generates compliance reports
//! 3. Flags high-risk transactions
//!
//! Run with: `cargo run -p simbank-examples --example 04_auditor_aml_scan`

use rust_decimal_macros::dec;
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};

fn main() {
    println!("=== Example 04: Auditor AML Scan ===\n");

    // =========================================================================
    // Part 1: Define auditor scenario
    // =========================================================================

    println!("📋 Defining AML audit scenario...\n");

    let scenario = banking_scenario! {
        // External auditor conducting quarterly review
        Auditor "Deloitte External" {
            scan from "2025-10-01" to "2025-12-31" flags ["large_amount", "near_threshold"];
            report Markdown;
        }

        // Internal compliance officer
        Auditor "Internal Compliance" {
            scan from "2025-01-01" flags ["high_risk_country"];
            report Json;
        }
    };

    println!("Scenario created with {} auditor blocks", scenario.blocks.len());
    println!();

    // Display auditor operations
    println!("🔍 Auditor Operations:\n");
    for (name, ops) in scenario.auditors() {
        println!("  Auditor: {}", name);
        for op in ops {
            match &op {
                simbank_dsl::AuditorOp::Scan { from_date, to_date, flags } => {
                    let date_range = match (from_date, to_date) {
                        (Some(from), Some(to)) => format!("{} to {}", from, to),
                        (Some(from), None) => format!("{} onwards", from),
                        _ => "All time".to_string(),
                    };
                    println!("    📅 Scan Period: {}", date_range);
                    println!("    🏷️  Filter Flags: {:?}", flags);
                }
                simbank_dsl::AuditorOp::Report { format } => {
                    println!("    📄 Report Format: {}", format);
                }
            }
        }
        println!();
    }

    // =========================================================================
    // Part 2: Define AML rules
    // =========================================================================

    println!("⚖️  AML Detection Rules:\n");

    let large_tx_rule = rule! {
        name: "Large Cash Transaction"
        when amount > 10000 USD
        then flag_aml "large_amount"
    };

    let near_threshold_rule = rule! {
        name: "Structuring Detection"
        when amount >= 9000 USD
        then flag_aml "near_threshold"
    };

    let high_risk_rule = rule! {
        name: "High Risk Country"
        when location in ["IR", "KP", "SY", "CU"]
        then flag_aml "high_risk_country"
    };

    let approval_rule = rule! {
        name: "New Account Large Transaction"
        when amount > 5000 USD
        then require_approval
    };

    println!("  📌 {}: {:?}", large_tx_rule.name, large_tx_rule.condition);
    println!("  📌 {}: {:?}", near_threshold_rule.name, near_threshold_rule.condition);
    println!("  📌 {}: {:?}", high_risk_rule.name, high_risk_rule.condition);
    println!("  📌 {}: {:?}", approval_rule.name, approval_rule.condition);
    println!();

    // Create RuleSet
    let ruleset = RuleSet::new()
        .add(large_tx_rule)
        .add(near_threshold_rule)
        .add(high_risk_rule)
        .add(approval_rule);

    // =========================================================================
    // Part 3: Simulate transaction scanning
    // =========================================================================

    println!("📊 Transaction Scan Results:\n");

    // Simulated transactions to scan
    let transactions = vec![
        ("TXN_001", "Alice", dec!(500), "USD", "deposit", "US"),
        ("TXN_002", "Bob", dec!(9500), "USD", "deposit", "US"),       // Near threshold!
        ("TXN_003", "Carol", dec!(15000), "USD", "withdrawal", "US"), // Large amount!
        ("TXN_004", "David", dec!(2000), "USD", "transfer", "IR"),    // High risk country!
        ("TXN_005", "Eve", dec!(8000), "USD", "deposit", "US"),       // Needs approval
        ("TXN_006", "Frank", dec!(100), "USD", "deposit", "RU"),      // Normal
        ("TXN_007", "Grace", dec!(50000), "USD", "withdrawal", "KP"), // Multiple flags!
    ];

    println!("  {:<10} {:<10} {:>12} {:<6} {:<12} {:<6} {}",
             "TX ID", "CUSTOMER", "AMOUNT", "CUR", "TYPE", "LOC", "FLAGS");
    println!("  {}", "─".repeat(75));

    let mut flagged_count = 0;

    for (tx_id, customer, amount, currency, tx_type, location) in &transactions {
        let ctx = TransactionContext::new()
            .with_amount(*amount, currency)
            .with_tx_type(tx_type)
            .with_location(location);

        let actions = ruleset.evaluate(&ctx);
        let flag_str = if actions.is_empty() {
            "✅".to_string()
        } else {
            flagged_count += 1;
            format!("⚠️  {} rules", actions.len())
        };

        println!("  {:<10} {:<10} {:>12} {:<6} {:<12} {:<6} {}",
                 tx_id, customer, amount, currency, tx_type, location, flag_str);
    }

    println!("  {}", "─".repeat(75));
    println!("  Total: {} transactions, {} flagged ({:.1}%)",
             transactions.len(),
             flagged_count,
             (flagged_count as f64 / transactions.len() as f64) * 100.0);
    println!();

    // =========================================================================
    // Part 4: Risk assessment summary
    // =========================================================================

    println!("🎯 Risk Assessment Summary:\n");

    // Calculate risk metrics
    let large_amount_count = transactions.iter()
        .filter(|(_, _, amount, _, _, _)| *amount > dec!(10000))
        .count();
    let near_threshold_count = transactions.iter()
        .filter(|(_, _, amount, _, _, _)| *amount >= dec!(9000) && *amount <= dec!(10000))
        .count();
    let high_risk_country_count = transactions.iter()
        .filter(|(_, _, _, _, _, loc)| ["IR", "KP", "SY", "CU"].contains(loc))
        .count();

    let risk_score = (large_amount_count * 30 + near_threshold_count * 20 + high_risk_country_count * 50) as f64
        / transactions.len() as f64;

    let risk_level = if risk_score < 20.0 {
        ("🟢", "Low")
    } else if risk_score < 40.0 {
        ("🟡", "Medium")
    } else if risk_score < 60.0 {
        ("🟠", "High")
    } else {
        ("🔴", "Critical")
    };

    println!("  Overall Risk Level: {} {}", risk_level.0, risk_level.1);
    println!("  Risk Score: {:.1}/100", risk_score);
    println!();

    println!("  Flag Breakdown:");
    println!("    🔸 Large Amount (>$10K):      {}", large_amount_count);
    println!("    🔸 Near Threshold ($9K-$10K): {}", near_threshold_count);
    println!("    🔸 High Risk Country:         {}", high_risk_country_count);
    println!();

    println!("  Recommendations:");
    if high_risk_country_count > 0 {
        println!("    ⚠️  Review {} transactions from high-risk jurisdictions",
                 high_risk_country_count);
    }
    if large_amount_count > 0 {
        println!("    ⚠️  File CTRs for {} large cash transactions",
                 large_amount_count);
    }
    if near_threshold_count > 0 {
        println!("    ⚠️  Investigate {} potential structuring attempts",
                 near_threshold_count);
    }
    println!();

    println!("✅ AML audit example completed!");
}
