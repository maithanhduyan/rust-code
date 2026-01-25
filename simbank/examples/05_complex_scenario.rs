//! # Example 05: Complex Multi-Stakeholder Scenario
//!
//! This example demonstrates a complete business scenario with all stakeholders:
//! 1. Customers performing daily banking
//! 2. Employees receiving payroll
//! 3. Shareholders receiving dividends
//! 4. Managers approving operations
//! 5. Auditors performing compliance checks
//!
//! Run with: `cargo run -p simbank-examples --example 05_complex_scenario`

use rust_decimal_macros::dec;
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║      Example 05: Complex Multi-Stakeholder Banking Scenario      ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // =========================================================================
    // Complete Scenario: Q4 2025 Corporate Operations
    // =========================================================================

    let scenario = banking_scenario! {
        // -------------------------------------
        // Customer Operations
        // -------------------------------------

        // High-value customer - Private Banking
        Customer "Alice Premium" {
            deposit 50000 USD to Funding;
            transfer 30000 USD from Funding to Spot;
            withdraw 5000 USD from Funding;
        }

        // Regular customer - Retail Banking
        Customer "Bob Regular" {
            deposit 2000 USD to Funding;
            transfer 500 USD from Funding to Spot;
            withdraw 200 USD from Funding;
        }

        // International customer
        Customer "Carlos International" {
            deposit 10000 EUR to Funding;
            transfer 8000 EUR from Funding to Spot;
        }

        // -------------------------------------
        // Employee Payroll
        // -------------------------------------

        // Senior Engineer
        Employee "Diana Dev" {
            receive_salary 12000 USD;
            buy_insurance "Premium Health" for 400 USD;
        }

        // Junior Analyst
        Employee "Eric Analyst" {
            receive_salary 5500 USD;
            buy_insurance "Basic Health" for 150 USD;
        }

        // Contractor
        Employee "Fiona Contractor" {
            receive_salary 8000 USD;
        }

        // -------------------------------------
        // Shareholder Dividends
        // -------------------------------------

        // Founding shareholders
        Shareholder "George Founder" {
            receive_dividend 25000 USD;
        }

        Shareholder "Helen Cofounder" {
            receive_dividend 20000 USD;
        }

        // Institutional investor
        Shareholder "Ivy Investments LLC" {
            receive_dividend 50000 USD;
        }

        // -------------------------------------
        // Manager Approvals
        // -------------------------------------

        Manager "James CEO" {
            pay_salary to "Diana Dev" amount 12000 USD;
            pay_bonus to "Diana Dev" amount 6000 USD reason "Year-end bonus";
        }

        Manager "Karen CFO" {
            pay_dividend to "George Founder" amount 25000 USD;
            pay_dividend to "Helen Cofounder" amount 20000 USD;
            pay_dividend to "Ivy Investments LLC" amount 50000 USD;
        }

        // -------------------------------------
        // Auditor Compliance
        // -------------------------------------

        Auditor "Compliance Team" {
            scan from "2025-10-01" to "2025-12-31" flags ["large_amount", "near_threshold"];
            report Markdown;
        }

        Auditor "External Audit" {
            scan from "2025-01-01" flags ["high_risk_country"];
            report Json;
        }
    };

    println!("📋 Scenario created with {} stakeholder blocks\n", scenario.blocks.len());

    // =========================================================================
    // Part 1: Customer Operations Summary
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                    CUSTOMER OPERATIONS                       │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    let mut total_deposits = dec!(0);
    let mut total_transfers = dec!(0);
    let mut total_withdrawals = dec!(0);

    for (name, ops) in scenario.customers() {
        println!("  👤 Customer: {}", name);
        for op in ops {
            match &op {
                simbank_dsl::CustomerOp::Deposit { amount, currency, .. } => {
                    total_deposits += amount;
                    println!("      💰 Deposit {} {}", amount, currency);
                }
                simbank_dsl::CustomerOp::Transfer { amount, currency, from_wallet, to_wallet } => {
                    total_transfers += amount;
                    println!("      ↗️  Transfer {} {} {:?} → {:?}", amount, currency, from_wallet, to_wallet);
                }
                simbank_dsl::CustomerOp::Withdraw { amount, currency, .. } => {
                    total_withdrawals += amount;
                    println!("      🏧 Withdraw {} {}", amount, currency);
                }
            }
        }
        println!();
    }

    println!("  📊 Customer Summary:");
    println!("     Total Deposits:    ${:>12}", total_deposits);
    println!("     Total Transfers:   ${:>12}", total_transfers);
    println!("     Total Withdrawals: ${:>12}", total_withdrawals);
    println!("     Net Change:        ${:>12}", total_deposits - total_withdrawals);
    println!();

    // =========================================================================
    // Part 2: Payroll Summary
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                    PAYROLL OPERATIONS                        │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    let mut total_salary = dec!(0);
    let mut total_insurance = dec!(0);

    for (name, ops) in scenario.employees() {
        println!("  👔 Employee: {}", name);
        let mut emp_total = dec!(0);
        for op in ops {
            match &op {
                simbank_dsl::EmployeeOp::ReceiveSalary { amount, currency } => {
                    total_salary += amount;
                    emp_total += amount;
                    println!("      💵 Salary: {} {}", amount, currency);
                }
                simbank_dsl::EmployeeOp::BuyInsurance { plan, cost, currency } => {
                    total_insurance += cost;
                    println!("      🏥 Insurance ({}): {} {}", plan, cost, currency);
                }
            }
        }
        println!("      ────────────────────────");
        println!("      Total Compensation: ${}", emp_total);
        println!();
    }

    println!("  📊 Payroll Summary:");
    println!("     Total Salaries:   ${:>12}", total_salary);
    println!("     Total Insurance:  ${:>12}", total_insurance);
    println!("     Gross Payroll:    ${:>12}", total_salary);
    println!();

    // =========================================================================
    // Part 3: Dividend Distribution
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                  DIVIDEND DISTRIBUTION                       │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    let mut total_dividends = dec!(0);
    let withholding_rate = dec!(0.15); // 15% tax withholding

    for (name, ops) in scenario.shareholders() {
        println!("  🏦 Shareholder: {}", name);
        for op in ops {
            if let simbank_dsl::ShareholderOp::ReceiveDividend { amount, currency } = &op {
                total_dividends += amount;
                let tax = amount * withholding_rate;
                let net = amount - tax;
                println!("      💵 Gross:  {} {}", amount, currency);
                println!("      🏛️  Tax:    {} {} (15%)", tax.round_dp(2), currency);
                println!("      💰 Net:    {} {}", net.round_dp(2), currency);
            }
        }
        println!();
    }

    println!("  📊 Dividend Summary:");
    println!("     Gross Dividends:   ${:>12}", total_dividends);
    println!("     Tax Withheld:      ${:>12}", (total_dividends * withholding_rate).round_dp(2));
    println!("     Net Distribution:  ${:>12}", (total_dividends * (dec!(1) - withholding_rate)).round_dp(2));
    println!();

    // =========================================================================
    // Part 4: Manager Operations
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                    MANAGER OPERATIONS                        │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    for (name, ops) in scenario.managers() {
        println!("  👔 Manager: {}", name);
        for op in ops {
            match &op {
                simbank_dsl::ManagerOp::PaySalary { employee_account, amount, currency } => {
                    println!("      💵 Pay Salary: {} {} to {}", amount, currency, employee_account);
                }
                simbank_dsl::ManagerOp::PayBonus { employee_account, amount, currency, reason } => {
                    println!("      🎁 Pay Bonus: {} {} to {} ({})", amount, currency, employee_account, reason);
                }
                simbank_dsl::ManagerOp::PayDividend { shareholder_account, amount, currency } => {
                    println!("      📈 Pay Dividend: {} {} to {}", amount, currency, shareholder_account);
                }
            }
        }
        println!();
    }

    // =========================================================================
    // Part 5: Compliance & Rules
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                  COMPLIANCE RULES CHECK                      │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    // Define comprehensive rules
    let ruleset = RuleSet::new()
        .add(rule! {
            name: "AML - Large Transaction"
            when amount > 10000 USD
            then flag_aml "large_amount"
        })
        .add(rule! {
            name: "AML - Near Threshold"
            when amount >= 9000 USD
            then flag_aml "near_threshold"
        })
        .add(rule! {
            name: "Withdrawal Limit"
            when amount > 50000 USD
            then require_approval
        })
        .add(rule! {
            name: "Daily Transfer Limit"
            when amount > 25000 USD
            then require_approval
        });

    // Check key transactions
    let check_transactions = vec![
        ("Alice Premium Deposit", dec!(50000), "USD", "deposit"),
        ("Ivy Investments Dividend", dec!(50000), "USD", "dividend"),
        ("Carlos International Transfer", dec!(8000), "EUR", "transfer"),
        ("Bob Regular Deposit", dec!(2000), "USD", "deposit"),
    ];

    println!("  Transaction Compliance Checks:\n");
    println!("  {:<30} {:>12} {:<8} {}", "TRANSACTION", "AMOUNT", "CUR", "STATUS");
    println!("  {}", "─".repeat(65));

    for (desc, amount, currency, tx_type) in check_transactions {
        let ctx = TransactionContext::new()
            .with_amount(amount, currency)
            .with_tx_type(tx_type)
            .with_location("US");

        let actions = ruleset.evaluate(&ctx);
        let status = if actions.is_empty() {
            "✅ Approved".to_string()
        } else {
            format!("⚠️  {} rules triggered", actions.len())
        };

        println!("  {:<30} {:>12} {:<8} {}", desc, amount, currency, status);
    }
    println!();

    // =========================================================================
    // Part 6: Financial Summary
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                    Q4 2025 FINANCIAL SUMMARY                 │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    let total_outflows = total_withdrawals + total_salary + total_dividends;

    println!("  📈 INFLOWS:");
    println!("     Customer Deposits:     ${:>12}", total_deposits);
    println!();

    println!("  📉 OUTFLOWS:");
    println!("     Customer Withdrawals:  ${:>12}", total_withdrawals);
    println!("     Payroll (Salary):      ${:>12}", total_salary);
    println!("     Dividend Payments:     ${:>12}", total_dividends);
    println!("     ─────────────────────────────────");
    println!("     Total Outflows:        ${:>12}", total_outflows);
    println!();

    println!("  💼 NET POSITION:");
    let net_cash = total_deposits - total_outflows;
    let net_emoji = if net_cash >= dec!(0) { "🟢" } else { "🔴" };
    println!("     {} Net Cash Flow:        ${:>12}", net_emoji, net_cash);
    println!();

    // =========================================================================
    // Part 7: Audit Trail
    // =========================================================================

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│                       AUDIT TRAIL                            │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    for (name, ops) in scenario.auditors() {
        println!("  🔍 Auditor: {}", name);
        for op in ops {
            match &op {
                simbank_dsl::AuditorOp::Scan { from_date, to_date, flags } => {
                    let date_range = match (from_date, to_date) {
                        (Some(from), Some(to)) => format!("{} to {}", from, to),
                        (Some(from), None) => format!("{} onwards", from),
                        _ => "All time".to_string(),
                    };
                    println!("      📅 Scan: {} | Flags: {:?}", date_range, flags);
                }
                simbank_dsl::AuditorOp::Report { format } => {
                    println!("      📄 Report: {}", format);
                }
            }
        }
        println!();
    }

    // Count operations
    let customer_count = scenario.customers().count();
    let employee_count = scenario.employees().count();
    let shareholder_count = scenario.shareholders().count();
    let manager_count = scenario.managers().count();
    let auditor_count = scenario.auditors().count();

    println!("  📊 Scenario Statistics:");
    println!("     Customers:    {}", customer_count);
    println!("     Employees:    {}", employee_count);
    println!("     Shareholders: {}", shareholder_count);
    println!("     Managers:     {}", manager_count);
    println!("     Auditors:     {}", auditor_count);
    println!("     ────────────────────");
    println!("     Total Actors: {}",
             customer_count + employee_count + shareholder_count + manager_count + auditor_count);
    println!();

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           ✅ Complex Scenario Example Completed!                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
