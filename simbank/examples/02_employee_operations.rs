//! # Example 02: Employee Operations
//!
//! This example demonstrates employee payroll operations:
//! 1. Receiving salary payments
//! 2. Buying insurance plans
//!
//! Run with: `cargo run -p simbank-examples --example 02_employee_operations`

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use simbank_dsl::{banking_scenario, rule, RuleSet, TransactionContext};

fn main() {
    println!("=== Example 02: Employee Operations ===\n");

    // =========================================================================
    // Part 1: Define employee scenario
    // =========================================================================

    println!("📋 Defining employee payroll scenario...\n");

    let scenario = banking_scenario! {
        // Senior Engineer - Full benefits
        Employee "Bob Engineer" {
            receive_salary 8000 USD;
            buy_insurance "Gold Plan" for 500 USD;
        }

        // Junior Analyst
        Employee "Carol Analyst" {
            receive_salary 5000 USD;
            buy_insurance "Silver Plan" for 300 USD;
        }

        // Intern with basic salary
        Employee "David Intern" {
            receive_salary 2500 USD;
            buy_insurance "Basic Plan" for 100 USD;
        }
    };

    println!("Scenario created with {} employee blocks", scenario.blocks.len());
    println!();

    // =========================================================================
    // Part 2: Display and calculate payroll
    // =========================================================================

    println!("👔 Employee Payroll Summary:\n");

    let mut total_salary = dec!(0);
    let mut total_insurance = dec!(0);
    let mut employee_count = 0;

    for (name, ops) in scenario.employees() {
        println!("  Employee: {}", name);
        let mut emp_salary = dec!(0);
        let mut emp_insurance = dec!(0);

        for op in ops {
            match &op {
                simbank_dsl::EmployeeOp::ReceiveSalary { amount, currency } => {
                    emp_salary = *amount;
                    total_salary += amount;
                    println!("    💵 Salary: {} {}", amount, currency);
                }
                simbank_dsl::EmployeeOp::BuyInsurance { plan, cost, currency } => {
                    emp_insurance = *cost;
                    total_insurance += cost;
                    println!("    🏥 Insurance ({}): {} {}", plan, cost, currency);
                }
            }
        }

        let net = emp_salary - emp_insurance;
        println!("    ────────────────────────────");
        println!("    Net Take-Home: {} USD", net);
        println!();

        employee_count += 1;
    }

    println!("  📊 Payroll Summary:");
    println!("     Employees:        {}", employee_count);
    println!("     Total Salary:     {} USD", total_salary);
    println!("     Total Insurance:  {} USD", total_insurance);
    println!("     Employer Cost:    {} USD", total_salary);
    println!();

    // =========================================================================
    // Part 3: Define payroll rules
    // =========================================================================

    println!("⚖️  Payroll Rules:\n");

    let salary_rule = rule! {
        name: "Salary Payment Approval"
        when amount > 5000 USD
        then require_approval
    };

    let bonus_rule = rule! {
        name: "Large Bonus AML"
        when amount > 10000 USD
        then flag_aml "large_bonus"
    };

    let insurance_rule = rule! {
        name: "Insurance Limit Check"
        when amount > 1000 USD
        then require_approval
    };

    println!("  📌 {}: {:?}", salary_rule.name, salary_rule.condition);
    println!("  📌 {}: {:?}", bonus_rule.name, bonus_rule.condition);
    println!("  📌 {}: {:?}", insurance_rule.name, insurance_rule.condition);
    println!();

    // =========================================================================
    // Part 4: Validate payroll transactions
    // =========================================================================

    println!("🔍 Payroll Validation:\n");

    let ruleset = RuleSet::new()
        .add(salary_rule)
        .add(bonus_rule)
        .add(insurance_rule);

    let test_payments = vec![
        ("Bob salary", dec!(8000), "salary"),
        ("Carol salary", dec!(5000), "salary"),
        ("David salary", dec!(2500), "salary"),
        ("Year-end bonus", dec!(15000), "bonus"),
    ];

    for (desc, amount, tx_type) in test_payments {
        let ctx = TransactionContext::new()
            .with_amount(amount, "USD")
            .with_tx_type(tx_type)
            .with_location("US");

        let actions = ruleset.evaluate(&ctx);
        let status = if actions.is_empty() {
            "✅ Auto-approved"
        } else {
            "⚠️  Needs Review"
        };

        println!("  {} (${}) : {} ({} rules)", desc, amount, status, actions.len());
    }
    println!();

    // =========================================================================
    // Part 5: Annual compensation report
    // =========================================================================

    println!("📈 Annual Compensation Report:\n");

    let monthly_salary = total_salary;
    let annual_salary = monthly_salary * Decimal::from(12);
    let annual_insurance = total_insurance * Decimal::from(12);

    println!("  Monthly Payroll:      ${:>12}", monthly_salary);
    println!("  Annual Payroll:       ${:>12}", annual_salary);
    println!("  Annual Insurance:     ${:>12}", annual_insurance);
    println!("  Total Annual Cost:    ${:>12}", annual_salary + annual_insurance);
    println!();

    println!("✅ Employee operations example completed!");
}
