//! # Example 01: Customer Onboarding
//!
//! This example demonstrates a typical customer onboarding workflow:
//! 1. Initial deposit to Funding wallet
//! 2. Transfer funds between wallets (Funding → Spot)
//! 3. Withdrawal from Funding wallet
//!
//! Run with: `cargo run -p simbank-examples --example 01_customer_onboarding`

use rust_decimal_macros::dec;
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};

fn main() {
    println!("=== Example 01: Customer Onboarding ===\n");

    // =========================================================================
    // Part 1: Define the banking scenario using DSL
    // =========================================================================

    println!("📋 Defining scenario with banking_scenario! macro...\n");

    let scenario = banking_scenario! {
        // Alice - High-value customer opening account
        Customer "Alice Premium" {
            deposit 1000 USDT to Funding;
            transfer 500 USDT from Funding to Spot;
            withdraw 200 USDT from Funding;
        }

        // Bob - Regular customer with simpler workflow
        Customer "Bob Regular" {
            deposit 500 USD to Funding;
            transfer 200 USD from Funding to Spot;
        }
    };

    println!("Scenario created with {} stakeholder blocks", scenario.blocks.len());
    println!();

    // =========================================================================
    // Part 2: Iterate and display operations
    // =========================================================================

    println!("👥 Customer Operations:\n");

    for (name, ops) in scenario.customers() {
        println!("  Customer: {}", name);
        for op in ops {
            match &op {
                simbank_dsl::CustomerOp::Deposit { amount, currency, to_wallet } => {
                    println!("    💰 Deposit {} {} to {:?}", amount, currency, to_wallet);
                }
                simbank_dsl::CustomerOp::Transfer { amount, currency, from_wallet, to_wallet } => {
                    println!("    ↗️  Transfer {} {} from {:?} to {:?}",
                             amount, currency, from_wallet, to_wallet);
                }
                simbank_dsl::CustomerOp::Withdraw { amount, currency, from_wallet } => {
                    println!("    🏧 Withdraw {} {} from {:?}", amount, currency, from_wallet);
                }
            }
        }
        println!();
    }

    // =========================================================================
    // Part 3: Define business rules using rule! macro
    // =========================================================================

    println!("⚖️  Business Rules:\n");

    // AML Rule: Flag transactions over $10,000
    let aml_rule = rule! {
        name: "Large Deposit AML"
        when amount > 10000 USD
        then flag_aml "large_amount"
    };

    println!("  📌 Rule: {}", aml_rule.name);
    println!("     Condition: {:?}", aml_rule.condition);
    println!("     Action: {:?}", aml_rule.action);
    println!();

    // Daily limit rule
    let limit_rule = rule! {
        name: "Daily Withdrawal Limit"
        when amount > 5000 USD
        then require_approval
    };

    println!("  📌 Rule: {}", limit_rule.name);
    println!("     Condition: {:?}", limit_rule.condition);
    println!("     Action: {:?}", limit_rule.action);
    println!();

    // =========================================================================
    // Part 4: Evaluate rules against sample transactions
    // =========================================================================

    println!("🔍 Rule Evaluation:\n");

    // Create RuleSet
    let ruleset = RuleSet::new()
        .add(aml_rule)
        .add(limit_rule);

    // Sample transactions to evaluate
    let test_transactions = vec![
        ("Small deposit", dec!(500), "USD", "deposit"),
        ("Large deposit", dec!(15000), "USD", "deposit"),
        ("Normal withdrawal", dec!(1000), "USD", "withdrawal"),
        ("Large withdrawal", dec!(7500), "USD", "withdrawal"),
    ];

    for (desc, amount, currency, tx_type) in test_transactions {
        let ctx = TransactionContext::new()
            .with_amount(amount, currency)
            .with_tx_type(tx_type)
            .with_location("US");

        let actions = ruleset.evaluate(&ctx);
        let status = if actions.is_empty() {
            "✅ Approved"
        } else {
            "⚠️  Review Required"
        };

        println!("  {} - {} {} {}: {} ({} rules matched)",
                 desc, amount, currency, tx_type, status, actions.len());
    }
    println!();

    println!("✅ Customer onboarding example completed!");
}
