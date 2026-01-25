//! # Example 03: Shareholder Dividends
//!
//! This example demonstrates dividend distribution:
//! 1. Quarterly dividend payments to shareholders
//! 2. Tax withholding calculations
//! 3. AML compliance for large payments
//!
//! Run with: `cargo run -p simbank-examples --example 03_shareholder_dividends`

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};

fn main() {
    println!("=== Example 03: Shareholder Dividends ===\n");

    // =========================================================================
    // Part 1: Define dividend distribution scenario
    // =========================================================================

    println!("📋 Defining Q4 2025 dividend distribution...\n");

    let scenario = banking_scenario! {
        // Founding shareholder - 25% ownership
        Shareholder "George Founder" {
            receive_dividend 50000 USD;
        }

        // Institutional investor - 50% ownership
        Shareholder "Ivy Investments LLC" {
            receive_dividend 100000 USD;
        }

        // Angel investor - 15% ownership
        Shareholder "Henry Angel" {
            receive_dividend 30000 USD;
        }

        // Early employee shareholder - 10% ownership
        Shareholder "Diana Early" {
            receive_dividend 20000 USD;
        }
    };

    println!("Scenario created with {} shareholder blocks", scenario.blocks.len());
    println!();

    // =========================================================================
    // Part 2: Calculate dividend distribution with taxes
    // =========================================================================

    println!("💰 Dividend Distribution (Q4 2025):\n");

    let withholding_rate = dec!(0.15); // 15% tax withholding
    let mut total_gross = dec!(0);
    let mut total_tax = dec!(0);
    let mut total_net = dec!(0);

    // Ownership percentages
    let ownership = vec![
        ("George Founder", dec!(25)),
        ("Ivy Investments LLC", dec!(50)),
        ("Henry Angel", dec!(15)),
        ("Diana Early", dec!(10)),
    ];
    let mut ownership_idx = 0;

    for (name, ops) in scenario.shareholders() {
        println!("  🏦 Shareholder: {}", name);

        for op in ops {
            if let simbank_dsl::ShareholderOp::ReceiveDividend { amount, currency } = op {
                let tax = amount * withholding_rate;
                let net = amount - tax;

                total_gross += amount;
                total_tax += tax;
                total_net += net;

                let (_, ownership_pct) = ownership[ownership_idx];
                ownership_idx += 1;

                println!("     Ownership:   {}%", ownership_pct);
                println!("     Gross:       {} {}", amount, currency);
                println!("     Tax (15%):   {} {}", tax.round_dp(2), currency);
                println!("     Net:         {} {}", net.round_dp(2), currency);
            }
        }
        println!();
    }

    println!("  📊 Distribution Summary:");
    println!("     Total Gross:      ${:>12}", total_gross);
    println!("     Total Tax:        ${:>12}", total_tax.round_dp(2));
    println!("     Total Net:        ${:>12}", total_net.round_dp(2));
    println!();

    // =========================================================================
    // Part 3: Define dividend compliance rules
    // =========================================================================

    println!("⚖️  Dividend Compliance Rules:\n");

    let large_dividend_rule = rule! {
        name: "Large Dividend Reporting"
        when amount > 50000 USD
        then flag_aml "large_amount"
    };

    let foreign_investor_rule = rule! {
        name: "Foreign Investor Withholding"
        when amount > 10000 USD
        then require_approval
    };

    let quarterly_cap_rule = rule! {
        name: "Quarterly Dividend Cap"
        when amount > 200000 USD
        then block
    };

    println!("  📌 {}", large_dividend_rule.name);
    println!("     Condition: {:?}", large_dividend_rule.condition);
    println!("     Action: {:?}", large_dividend_rule.action);
    println!();

    println!("  📌 {}", foreign_investor_rule.name);
    println!("     Condition: {:?}", foreign_investor_rule.condition);
    println!("     Action: {:?}", foreign_investor_rule.action);
    println!();

    println!("  📌 {}", quarterly_cap_rule.name);
    println!("     Condition: {:?}", quarterly_cap_rule.condition);
    println!("     Action: {:?}", quarterly_cap_rule.action);
    println!();

    // =========================================================================
    // Part 4: Evaluate dividends against rules
    // =========================================================================

    println!("🔍 Compliance Check:\n");

    let ruleset = RuleSet::new()
        .add(large_dividend_rule)
        .add(foreign_investor_rule)
        .add(quarterly_cap_rule);

    let dividends = vec![
        ("George Founder", dec!(50000)),
        ("Ivy Investments LLC", dec!(100000)),
        ("Henry Angel", dec!(30000)),
        ("Diana Early", dec!(20000)),
    ];

    println!("  {:<25} {:>12} {:<20}", "SHAREHOLDER", "AMOUNT", "STATUS");
    println!("  {}", "─".repeat(60));

    for (name, amount) in dividends {
        let ctx = TransactionContext::new()
            .with_amount(amount, "USD")
            .with_tx_type("dividend")
            .with_location("US");

        let actions = ruleset.evaluate(&ctx);
        let status = if actions.is_empty() {
            "✅ Approved".to_string()
        } else if ruleset.is_blocked(&ctx) {
            "🚫 Blocked".to_string()
        } else {
            format!("⚠️  Review ({})", actions.len())
        };

        println!("  {:<25} ${:>11} {}", name, amount, status);
    }
    println!();

    // =========================================================================
    // Part 5: Tax reporting summary
    // =========================================================================

    println!("📄 Tax Reporting (1099-DIV):\n");

    println!("  Form 1099-DIV Summary for Tax Year 2025");
    println!("  ────────────────────────────────────────");
    println!("  Total Ordinary Dividends (1a):  ${}", total_gross);
    println!("  Total Qualified Dividends (1b): ${}", total_gross);
    println!("  Federal Tax Withheld (4):       ${}", total_tax.round_dp(2));
    println!();

    // Projected annual dividends
    let annual_projection = total_gross * Decimal::from(4);
    println!("  📈 Annual Projection (4 quarters):");
    println!("     Gross Dividends:    ${}", annual_projection);
    println!("     Total Withholding:  ${}", (total_tax * Decimal::from(4)).round_dp(2));
    println!();

    println!("✅ Shareholder dividends example completed!");
}
