# BiBank - Phase 4 Specification

> **Document Version:** 1.0
> **Date:** 2026-01-26
> **Status:** 📝 DRAFT - Open for Review
> **Author:** Team BiBank
> **Depends on:** Phase 3 (Complete ✅)
> **Target Reviewers:** GPT5, Gemini3

---

## 1. Tổng quan Phase 4

### 1.1 Mục tiêu

Phase 4 biến BiBank từ **margin-trading platform** thành **compliant financial infrastructure**:

1. **AML Real-time Hooks** - Anti-Money Laundering với rule-based detection
2. **Rule DSL** - Domain-specific language cho business rules
3. **Compliance Engine** - KYC/KYT integration points
4. **Audit Trail Enhancement** - Immutable compliance logs

### 1.2 Phase 3 Recap

| Component | Status |
|-----------|--------|
| Margin System (10x leverage) | ✅ |
| Borrow/Repay TransactionIntents | ✅ |
| Order Matching Engine (CLOB) | ✅ |
| Liquidation Engine | ✅ |
| Multi-signature Approval (2-of-3) | ✅ |
| Interest Accrual | ✅ |
| Oracle Price Feed | ✅ |
| 153 tests passing | ✅ |

### 1.3 Triết lý Phase 4

> **"Compliance by Design, Not Afterthought"**

Phase 4 tập trung vào:
- **Rule DSL** để BA/Compliance team có thể định nghĩa rules
- **Real-time AML** detection tại thời điểm transaction
- **Audit-ready** với compliance logs

---

## 2. Rule DSL Specification

### 2.1 Design Goals

1. **Readable by Non-Developers** - BA/Compliance team có thể đọc và review
2. **Compile-time Safe** - Rust macro system catches errors early
3. **Auditable** - Rules được hash và signed
4. **Versioned** - Rule changes tracked in ledger

### 2.2 DSL Syntax Overview

```rust
use bibank_dsl::*;

// Define a rule set
rule_set! {
    name: "AML_BASIC_V1",
    version: "1.0.0",

    rules: [
        // Rule 1: Large transaction alert
        rule!(
            name: "LARGE_TX_ALERT",
            description: "Flag transactions over 10,000 USDT",

            when: transaction.amount >= 10_000 USDT,
            then: {
                flag_for_review("Large transaction detected");
                notify_compliance_team();
            }
        ),

        // Rule 2: Rapid successive transactions
        rule!(
            name: "RAPID_TX_PATTERN",
            description: "Detect structuring attempts",

            when: user.transactions_in_last(1.hour) >= 5
              and user.total_volume_in_last(1.hour) >= 9_000 USDT,
            then: {
                flag_for_review("Possible structuring detected");
                set_risk_score(user, HIGH);
            }
        ),

        // Rule 3: New account large withdrawal
        rule!(
            name: "NEW_ACCOUNT_LARGE_WD",
            description: "New accounts with large withdrawals",

            when: user.account_age < 7.days
              and transaction.intent == Withdrawal
              and transaction.amount >= 5_000 USDT,
            then: {
                require_manual_approval();
                flag_for_review("New account large withdrawal");
            }
        ),
    ]
}
```

### 2.3 Rule Syntax Grammar

```ebnf
RuleSet     ::= 'rule_set!' '{'
                  'name:' STRING ','
                  'version:' STRING ','
                  'rules:' '[' Rule (',' Rule)* ']'
                '}'

Rule        ::= 'rule!' '('
                  'name:' STRING ','
                  'description:' STRING ','
                  'when:' Condition ','
                  'then:' ActionBlock
                ')'

Condition   ::= SimpleCondition
              | Condition 'and' Condition
              | Condition 'or' Condition
              | 'not' Condition
              | '(' Condition ')'

SimpleCondition ::= Field Comparator Value
                  | Field 'in' '[' Value (',' Value)* ']'
                  | FunctionCall

Field       ::= 'transaction.' FieldName
              | 'user.' FieldName
              | 'account.' FieldName

Comparator  ::= '==' | '!=' | '>' | '>=' | '<' | '<='

Value       ::= NUMBER
              | NUMBER CurrencyUnit
              | STRING
              | Duration
              | EnumVariant

Duration    ::= NUMBER '.' TimeUnit
TimeUnit    ::= 'seconds' | 'minutes' | 'hours' | 'days'

ActionBlock ::= '{' Action (';' Action)* '}'

Action      ::= 'flag_for_review' '(' STRING ')'
              | 'notify_compliance_team' '(' ')'
              | 'require_manual_approval' '(' ')'
              | 'set_risk_score' '(' Identifier ',' RiskLevel ')'
              | 'block_transaction' '(' STRING ')'
              | 'log_event' '(' STRING ')'

RiskLevel   ::= 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'
```

### 2.4 Banking Scenario DSL

Cho testing và documentation:

```rust
use bibank_dsl::banking_scenario;

banking_scenario! {
    name: "Margin Trading Happy Path",
    description: "User deposits, borrows, trades, repays",

    // Setup
    setup: {
        create_user("ALICE");
        set_price(BTC, 50_000 USDT);
    },

    // Scenario steps
    steps: [
        // Step 1: Deposit
        step!(
            actor: "ALICE",
            action: deposit(10_000 USDT),
            expect: {
                balance("ALICE", USDT) == 10_000,
            }
        ),

        // Step 2: Borrow with 5x leverage
        step!(
            actor: "ALICE",
            action: borrow(40_000 USDT),
            expect: {
                balance("ALICE", USDT) == 50_000,
                loan("ALICE", USDT) == 40_000,
                margin_ratio("ALICE") >= 1.25,
            }
        ),

        // Step 3: Buy BTC
        step!(
            actor: "ALICE",
            action: trade(buy 1 BTC at 50_000 USDT),
            expect: {
                balance("ALICE", BTC) == 1,
                balance("ALICE", USDT) == 0,
            }
        ),

        // Step 4: Price increases
        step!(
            action: set_price(BTC, 55_000 USDT),
            expect: {
                unrealized_pnl("ALICE") == 5_000 USDT,
            }
        ),

        // Step 5: Sell and repay
        step!(
            actor: "ALICE",
            action: trade(sell 1 BTC at 55_000 USDT),
            expect: {
                balance("ALICE", USDT) == 55_000,
            }
        ),

        step!(
            actor: "ALICE",
            action: repay(40_000 USDT),
            expect: {
                loan("ALICE", USDT) == 0,
                balance("ALICE", USDT) == 15_000, // 10k deposit + 5k profit
            }
        ),
    ],

    // Final assertions
    assert: {
        profit("ALICE") == 5_000 USDT,
        no_outstanding_loans(),
    }
}
```

### 2.5 Rule Compilation

Rules compile to Rust structs at compile time:

```rust
/// Generated from rule! macro
pub struct LargeTxAlertRule {
    pub name: &'static str,
    pub description: &'static str,
    pub hash: &'static str,  // SHA256 of rule source
}

impl Rule for LargeTxAlertRule {
    fn evaluate(&self, ctx: &RuleContext) -> RuleResult {
        // Generated condition check
        if ctx.transaction.amount >= Decimal::new(10_000, 0) {
            RuleResult::Triggered {
                actions: vec![
                    Action::FlagForReview("Large transaction detected".into()),
                    Action::NotifyComplianceTeam,
                ],
            }
        } else {
            RuleResult::Passed
        }
    }
}
```

---

## 3. AML Real-time Hooks

### 3.1 Hook Points

AML checks integrated at critical points:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Transaction Flow                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Intent Created ──► [PRE_VALIDATION_HOOK] ──► Validation        │
│                                                     │            │
│                                                     ▼            │
│                                              [AML_CHECK_HOOK]    │
│                                                     │            │
│                     ┌──────────────────────────────┴──────┐     │
│                     │                                      │     │
│                     ▼                                      ▼     │
│              [APPROVED]                            [FLAGGED]     │
│                     │                                      │     │
│                     ▼                                      ▼     │
│               Ledger Commit ◄─────────────── Manual Review      │
│                     │                                      │     │
│                     ▼                                      ▼     │
│           [POST_COMMIT_HOOK] ──────────► Compliance Log         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Hook Interface

```rust
/// Hook that runs before transaction validation
#[async_trait]
pub trait PreValidationHook: Send + Sync {
    /// Called before intent validation
    /// Return Err to reject transaction immediately
    async fn on_pre_validation(
        &self,
        intent: &UnsignedEntry,
        context: &HookContext,
    ) -> Result<HookDecision, HookError>;
}

/// Main AML check hook
#[async_trait]
pub trait AmlCheckHook: Send + Sync {
    /// Called after validation, before commit
    async fn on_aml_check(
        &self,
        entry: &UnsignedEntry,
        context: &AmlContext,
    ) -> AmlDecision;
}

/// Hook that runs after ledger commit
#[async_trait]
pub trait PostCommitHook: Send + Sync {
    /// Called after successful commit
    async fn on_post_commit(
        &self,
        entry: &JournalEntry,
        context: &HookContext,
    ) -> Result<(), HookError>;
}

/// Decision from AML check
pub enum AmlDecision {
    /// Transaction approved, continue
    Approved,

    /// Transaction flagged, requires manual review
    Flagged {
        reason: String,
        risk_score: RiskScore,
        required_approval: ApprovalLevel,
    },

    /// Transaction blocked
    Blocked {
        reason: String,
        compliance_code: String,
    },
}

/// Risk score levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskScore {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Approval level required
pub enum ApprovalLevel {
    /// Single compliance officer
    L1,
    /// Senior compliance officer
    L2,
    /// Head of compliance
    L3,
    /// Board level (for critical cases)
    L4,
}
```

### 3.3 AML Context

```rust
/// Context provided to AML hooks
pub struct AmlContext {
    /// The transaction being checked
    pub transaction: TransactionInfo,

    /// User information
    pub user: UserProfile,

    /// Historical data
    pub history: TransactionHistory,

    /// Current system state
    pub system_state: SystemState,
}

pub struct TransactionInfo {
    pub intent: TransactionIntent,
    pub amount: Decimal,
    pub asset: String,
    pub source_account: String,
    pub destination_account: Option<String>,
    pub correlation_id: String,
    pub timestamp: DateTime<Utc>,
}

pub struct UserProfile {
    pub user_id: String,
    pub kyc_level: KycLevel,
    pub account_age: Duration,
    pub risk_score: RiskScore,
    pub country: Option<String>,
    pub is_pep: bool,  // Politically Exposed Person
    pub watchlist_status: WatchlistStatus,
}

pub struct TransactionHistory {
    /// Transactions in last 24 hours
    pub last_24h: Vec<TransactionSummary>,

    /// Transactions in last 7 days
    pub last_7d: Vec<TransactionSummary>,

    /// Total volume in last 24h
    pub volume_24h: HashMap<String, Decimal>,

    /// Total volume in last 7d
    pub volume_7d: HashMap<String, Decimal>,

    /// Count of transactions by type in last 24h
    pub tx_count_24h: HashMap<TransactionIntent, u32>,
}
```

### 3.4 Built-in AML Rules

```rust
/// Standard AML rules shipped with BiBank
pub mod builtin_rules {
    use super::*;

    rule_set! {
        name: "BIBANK_AML_STANDARD_V1",
        version: "1.0.0",

        rules: [
            // === Threshold-based rules ===

            rule!(
                name: "CTR_THRESHOLD",
                description: "Currency Transaction Report threshold (10,000 USD equivalent)",
                when: transaction.usd_equivalent >= 10_000,
                then: {
                    log_event("CTR threshold reached");
                    generate_ctr_report();
                }
            ),

            rule!(
                name: "SAR_LARGE_CASH",
                description: "Suspicious Activity Report for large cash-like transactions",
                when: transaction.usd_equivalent >= 5_000
                  and transaction.intent in [Deposit, Withdrawal]
                  and transaction.is_crypto_to_fiat,
                then: {
                    flag_for_review("Large crypto-fiat conversion");
                    set_risk_score(user, MEDIUM);
                }
            ),

            // === Pattern-based rules ===

            rule!(
                name: "STRUCTURING_DETECTION",
                description: "Detect potential structuring (smurfing)",
                when: user.tx_count_24h >= 3
                  and user.volume_24h >= 8_000 USDT
                  and user.volume_24h < 10_000 USDT
                  and all_transactions_below(3_500 USDT),
                then: {
                    flag_for_review("Potential structuring pattern");
                    set_risk_score(user, HIGH);
                    require_manual_approval();
                }
            ),

            rule!(
                name: "RAPID_MOVEMENT",
                description: "Funds rapidly moved in and out",
                when: user.deposits_24h >= 5_000 USDT
                  and user.withdrawals_24h >= 4_000 USDT
                  and time_between_deposit_withdrawal < 1.hour,
                then: {
                    flag_for_review("Rapid fund movement - possible layering");
                    set_risk_score(user, HIGH);
                }
            ),

            // === KYC-based rules ===

            rule!(
                name: "UNVERIFIED_LARGE_TX",
                description: "Large transaction from unverified user",
                when: user.kyc_level == Unverified
                  and transaction.usd_equivalent >= 1_000,
                then: {
                    block_transaction("KYC required for transactions >= $1,000");
                }
            ),

            rule!(
                name: "PEP_MONITORING",
                description: "Enhanced monitoring for PEPs",
                when: user.is_pep == true,
                then: {
                    log_event("PEP transaction recorded");
                    if transaction.usd_equivalent >= 5_000 {
                        flag_for_review("PEP large transaction");
                    }
                }
            ),

            // === Velocity rules ===

            rule!(
                name: "NEW_ACCOUNT_VELOCITY",
                description: "New account with unusual activity",
                when: user.account_age < 7.days
                  and user.tx_count_24h >= 10,
                then: {
                    flag_for_review("New account unusual velocity");
                    set_risk_score(user, MEDIUM);
                }
            ),

            // === Watchlist rules ===

            rule!(
                name: "WATCHLIST_BLOCK",
                description: "Block transactions involving watchlisted entities",
                when: user.watchlist_status == Blocked
                   or counterparty.watchlist_status == Blocked,
                then: {
                    block_transaction("Entity on blocked list");
                    notify_compliance_team();
                    generate_sar_report();
                }
            ),
        ]
    }
}
```

---

## 4. Compliance Engine

### 4.1 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Compliance Engine                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ Rule Engine  │  │ KYC Service  │  │ Watchlist    │          │
│  │              │  │ (External)   │  │ Service      │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         └────────────┬────┴────────────────┘                   │
│                      │                                          │
│              ┌───────▼───────┐                                  │
│              │ Decision      │                                  │
│              │ Aggregator    │                                  │
│              └───────┬───────┘                                  │
│                      │                                          │
│         ┌────────────┼────────────┐                             │
│         │            │            │                             │
│         ▼            ▼            ▼                             │
│    ┌─────────┐ ┌─────────┐ ┌─────────┐                         │
│    │ Approve │ │ Flag    │ │ Block   │                         │
│    └─────────┘ └─────────┘ └─────────┘                         │
│                      │                                          │
│              ┌───────▼───────┐                                  │
│              │ Compliance    │                                  │
│              │ Log (SQLite)  │                                  │
│              └───────────────┘                                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Compliance Engine Interface

```rust
/// Main compliance engine
pub struct ComplianceEngine {
    /// Rule sets (can have multiple active)
    rule_sets: Vec<Box<dyn RuleSet>>,

    /// External KYC provider
    kyc_provider: Option<Box<dyn KycProvider>>,

    /// Watchlist provider
    watchlist_provider: Box<dyn WatchlistProvider>,

    /// Compliance log storage
    log_store: ComplianceLogStore,

    /// Configuration
    config: ComplianceConfig,
}

impl ComplianceEngine {
    /// Check a transaction against all rules
    pub async fn check_transaction(
        &self,
        entry: &UnsignedEntry,
        user_id: &str,
    ) -> ComplianceResult {
        // 1. Build context
        let context = self.build_context(entry, user_id).await?;

        // 2. Run all rule sets
        let mut results = Vec::new();
        for rule_set in &self.rule_sets {
            let result = rule_set.evaluate(&context).await;
            results.push(result);
        }

        // 3. Aggregate decisions
        let decision = self.aggregate_decisions(&results);

        // 4. Log to compliance store
        self.log_store.log_check(&context, &decision).await?;

        Ok(decision)
    }

    /// Add a new rule set
    pub fn add_rule_set(&mut self, rule_set: Box<dyn RuleSet>) {
        self.rule_sets.push(rule_set);
    }

    /// Get pending reviews
    pub async fn get_pending_reviews(&self) -> Vec<PendingReview> {
        self.log_store.get_pending_reviews().await
    }

    /// Resolve a flagged transaction
    pub async fn resolve_review(
        &self,
        review_id: &str,
        decision: ReviewDecision,
        reviewer_id: &str,
        notes: &str,
    ) -> Result<(), ComplianceError> {
        self.log_store.resolve_review(review_id, decision, reviewer_id, notes).await
    }
}
```

### 4.3 Compliance Log Schema

```sql
-- Compliance check logs
CREATE TABLE compliance_checks (
    id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    transaction_intent TEXT NOT NULL,
    amount TEXT NOT NULL,
    asset TEXT NOT NULL,

    -- Decision
    decision TEXT NOT NULL,  -- 'approved', 'flagged', 'blocked'
    risk_score INTEGER,

    -- Rules triggered
    rules_triggered_json TEXT,  -- JSON array of rule names

    -- Timestamps
    checked_at TEXT NOT NULL,

    -- Indexes
    FOREIGN KEY (correlation_id) REFERENCES journal_entries(correlation_id)
);

CREATE INDEX idx_compliance_checks_user ON compliance_checks(user_id);
CREATE INDEX idx_compliance_checks_decision ON compliance_checks(decision);
CREATE INDEX idx_compliance_checks_date ON compliance_checks(checked_at);

-- Pending reviews for flagged transactions
CREATE TABLE pending_reviews (
    id TEXT PRIMARY KEY,
    compliance_check_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,

    -- Flag details
    reason TEXT NOT NULL,
    risk_score INTEGER NOT NULL,
    required_approval_level INTEGER NOT NULL,

    -- Status
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'approved', 'rejected'

    -- Review info
    reviewed_by TEXT,
    reviewed_at TEXT,
    review_notes TEXT,

    -- Timestamps
    created_at TEXT NOT NULL,
    expires_at TEXT,  -- Auto-reject after timeout?

    FOREIGN KEY (compliance_check_id) REFERENCES compliance_checks(id)
);

CREATE INDEX idx_pending_reviews_status ON pending_reviews(status);
CREATE INDEX idx_pending_reviews_user ON pending_reviews(user_id);

-- Audit trail for rule changes
CREATE TABLE rule_audit (
    id TEXT PRIMARY KEY,
    rule_set_name TEXT NOT NULL,
    rule_set_version TEXT NOT NULL,
    rule_set_hash TEXT NOT NULL,

    action TEXT NOT NULL,  -- 'activated', 'deactivated', 'updated'
    performed_by TEXT NOT NULL,
    performed_at TEXT NOT NULL,

    -- Previous state (for rollback)
    previous_hash TEXT,

    -- Approval info
    approved_by_json TEXT  -- JSON array of approvers
);
```

---

## 5. KYC Integration

### 5.1 KYC Levels

```rust
/// KYC verification levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KycLevel {
    /// No verification
    Unverified = 0,

    /// Email verified
    EmailVerified = 1,

    /// Basic KYC (ID + selfie)
    Basic = 2,

    /// Enhanced KYC (ID + selfie + proof of address)
    Enhanced = 3,

    /// Full KYC (all above + source of funds)
    Full = 4,
}

/// Limits based on KYC level
pub struct KycLimits {
    /// Daily withdrawal limit
    pub daily_withdrawal: Decimal,

    /// Monthly withdrawal limit
    pub monthly_withdrawal: Decimal,

    /// Single transaction limit
    pub single_tx: Decimal,

    /// Features available
    pub features: HashSet<Feature>,
}

impl KycLevel {
    pub fn limits(&self) -> KycLimits {
        match self {
            KycLevel::Unverified => KycLimits {
                daily_withdrawal: Decimal::new(100, 0),
                monthly_withdrawal: Decimal::new(1_000, 0),
                single_tx: Decimal::new(100, 0),
                features: hashset!{ Feature::Deposit },
            },
            KycLevel::EmailVerified => KycLimits {
                daily_withdrawal: Decimal::new(1_000, 0),
                monthly_withdrawal: Decimal::new(10_000, 0),
                single_tx: Decimal::new(1_000, 0),
                features: hashset!{ Feature::Deposit, Feature::Withdraw, Feature::Trade },
            },
            KycLevel::Basic => KycLimits {
                daily_withdrawal: Decimal::new(10_000, 0),
                monthly_withdrawal: Decimal::new(100_000, 0),
                single_tx: Decimal::new(10_000, 0),
                features: hashset!{
                    Feature::Deposit, Feature::Withdraw,
                    Feature::Trade, Feature::Margin
                },
            },
            KycLevel::Enhanced => KycLimits {
                daily_withdrawal: Decimal::new(100_000, 0),
                monthly_withdrawal: Decimal::new(1_000_000, 0),
                single_tx: Decimal::new(100_000, 0),
                features: hashset!{
                    Feature::Deposit, Feature::Withdraw,
                    Feature::Trade, Feature::Margin,
                    Feature::Fiat
                },
            },
            KycLevel::Full => KycLimits {
                daily_withdrawal: Decimal::MAX,
                monthly_withdrawal: Decimal::MAX,
                single_tx: Decimal::MAX,
                features: hashset!{ Feature::All },
            },
        }
    }
}
```

### 5.2 KYC Provider Interface

```rust
/// External KYC provider interface
#[async_trait]
pub trait KycProvider: Send + Sync {
    /// Get current KYC status for user
    async fn get_kyc_status(&self, user_id: &str) -> Result<KycStatus, KycError>;

    /// Initiate KYC verification
    async fn initiate_verification(
        &self,
        user_id: &str,
        level: KycLevel,
    ) -> Result<VerificationSession, KycError>;

    /// Check verification status
    async fn check_verification(
        &self,
        session_id: &str,
    ) -> Result<VerificationResult, KycError>;
}

/// KYC status for a user
pub struct KycStatus {
    pub user_id: String,
    pub level: KycLevel,
    pub verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub documents: Vec<DocumentInfo>,
}
```

---

## 6. Crate Structure

### 6.1 New Crates

```
crates/
├── dsl/                     # UPDATED
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── rule.rs          # rule! macro
│       ├── rule_set.rs      # rule_set! macro
│       ├── scenario.rs      # banking_scenario! macro
│       ├── condition.rs     # Condition parsing
│       ├── action.rs        # Action definitions
│       └── compiler.rs      # Macro expansion
│
├── compliance/              # NEW CRATE
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── engine.rs        # ComplianceEngine
│       ├── aml.rs           # AML hooks
│       ├── kyc.rs           # KYC integration
│       ├── watchlist.rs     # Watchlist service
│       ├── rules/
│       │   ├── mod.rs
│       │   ├── builtin.rs   # Built-in AML rules
│       │   └── loader.rs    # Rule loading
│       └── store.rs         # Compliance log SQLite
│
├── hooks/                   # NEW CRATE
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── pre_validation.rs
│       ├── aml_check.rs
│       ├── post_commit.rs
│       └── registry.rs      # Hook registration
```

### 6.2 Updated Dependencies

```toml
# crates/dsl/Cargo.toml
[package]
name = "bibank-dsl"
version = "0.1.0"
edition = "2021"

[dependencies]
bibank-ledger.workspace = true
bibank-core.workspace = true
rust_decimal.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
sha2.workspace = true
hex.workspace = true

# For proc macros (if needed)
syn = { version = "2.0", features = ["full", "parsing"] }
quote = "1.0"
proc-macro2 = "1.0"

[dev-dependencies]
bibank-risk.workspace = true
```

---

## 7. CLI Commands

### 7.1 Compliance Commands

```bash
# === Rule Management ===

# List active rule sets
bibank compliance rules list

# Activate a rule set
bibank compliance rules activate --name "AML_BASIC_V1" --version "1.0.0"

# Deactivate a rule set
bibank compliance rules deactivate --name "AML_BASIC_V1"

# Show rule details
bibank compliance rules show --name "LARGE_TX_ALERT"

# === Review Management ===

# List pending reviews
bibank compliance reviews list [--status pending|approved|rejected]

# Show review details
bibank compliance reviews show --id REV-001

# Approve a flagged transaction
bibank compliance reviews approve --id REV-001 --notes "Verified with customer"

# Reject a flagged transaction
bibank compliance reviews reject --id REV-001 --notes "Suspicious activity confirmed"

# === Reports ===

# Generate compliance report
bibank compliance report --from 2026-01-01 --to 2026-01-31 --output report.pdf

# Export SAR report
bibank compliance sar --id SAR-001 --format xml

# === Watchlist ===

# Add to watchlist
bibank compliance watchlist add --user-id ALICE --reason "Internal investigation"

# Remove from watchlist
bibank compliance watchlist remove --user-id ALICE

# Check watchlist status
bibank compliance watchlist check --user-id ALICE
```

### 7.2 DSL Testing Commands

```bash
# Run a banking scenario
bibank scenario run --file scenarios/margin_happy_path.rs

# Validate DSL syntax
bibank scenario validate --file scenarios/test.rs

# Generate scenario report
bibank scenario report --file scenarios/test.rs --output report.html
```

---

## 8. Invariants & Rules

### 8.1 Compliance Invariants

| Rule | Description |
|------|-------------|
| **CTR Required** | Transactions >= $10,000 USD equivalent MUST generate CTR |
| **SAR Timebound** | SAR must be filed within 30 days of detection |
| **Audit Immutable** | Compliance logs cannot be modified, only appended |
| **Rule Versioned** | All rule changes tracked with hash and approvals |
| **Review Timeout** | Flagged transactions auto-expire after 72h if not reviewed |
| **KYC Enforced** | Transactions beyond KYC limits MUST be blocked |

### 8.2 DSL Invariants

| Rule | Description |
|------|-------------|
| **Compile-time Safe** | Invalid DSL syntax = compilation error |
| **Deterministic** | Same input + same rules = same output |
| **No Side Effects** | Rules evaluate state, don't modify it directly |
| **Auditable** | Every rule evaluation logged with rule hash |

---

## 9. Testing Strategy

### 9.1 Test Categories

```rust
#[cfg(test)]
mod tests {
    // === DSL Tests ===

    #[test]
    fn test_rule_macro_basic() {
        rule!(
            name: "TEST_RULE",
            description: "Test rule",
            when: transaction.amount >= 100 USDT,
            then: { flag_for_review("Test"); }
        );
        // Should compile without errors
    }

    #[test]
    fn test_rule_set_evaluation() {
        let rule_set = builtin_rules::aml_standard_v1();
        let ctx = create_test_context(amount: 15_000, intent: Deposit);

        let result = rule_set.evaluate(&ctx);

        assert!(result.triggered_rules.contains("CTR_THRESHOLD"));
    }

    // === AML Hook Tests ===

    #[test]
    fn test_large_tx_flagged() {
        let engine = ComplianceEngine::default();
        let entry = create_deposit_entry(15_000, "USDT");

        let result = engine.check_transaction(&entry, "ALICE").await;

        assert_eq!(result.decision, AmlDecision::Flagged { .. });
    }

    #[test]
    fn test_structuring_detection() {
        let engine = ComplianceEngine::default();

        // Simulate 5 transactions just under threshold
        for _ in 0..5 {
            let entry = create_deposit_entry(1_900, "USDT");
            engine.check_transaction(&entry, "ALICE").await;
        }

        let result = engine.check_transaction(
            &create_deposit_entry(1_900, "USDT"),
            "ALICE"
        ).await;

        assert!(result.rules_triggered.contains("STRUCTURING_DETECTION"));
    }

    // === Scenario Tests ===

    #[test]
    fn test_banking_scenario() {
        banking_scenario! {
            name: "Test Scenario",
            steps: [
                step!(actor: "ALICE", action: deposit(1_000 USDT)),
                step!(actor: "ALICE", action: withdraw(500 USDT)),
            ],
            assert: {
                balance("ALICE", USDT) == 500,
            }
        }
    }
}
```

### 9.2 Test Count Target

| Module | Estimated Tests |
|--------|-----------------|
| DSL rule! macro | 15 |
| DSL rule_set! macro | 10 |
| DSL banking_scenario! | 15 |
| AML hooks | 20 |
| Compliance engine | 25 |
| KYC integration | 10 |
| Built-in rules | 15 |
| CLI commands | 10 |
| **Total** | **120+** |

---

## 10. Migration Path

### 10.1 From Phase 3

```sql
-- New tables for Phase 4
CREATE TABLE compliance_checks (...);
CREATE TABLE pending_reviews (...);
CREATE TABLE rule_audit (...);

-- New indexes on existing tables
CREATE INDEX idx_journal_entries_user_intent
ON journal_entries(user_id, intent);
```

### 10.2 Feature Flags

```rust
/// Phase 4 features (gradual rollout)
pub struct Phase4Features {
    /// Enable AML hooks
    pub aml_hooks_enabled: bool,

    /// Enable rule DSL
    pub rule_dsl_enabled: bool,

    /// Compliance engine mode
    pub compliance_mode: ComplianceMode,
}

pub enum ComplianceMode {
    /// Log only, don't block
    Passive,
    /// Active enforcement
    Active,
}
```

---

## 11. Out of Scope (Phase 5+)

| Feature | Reason |
|---------|--------|
| Machine Learning AML | Requires data science expertise |
| Blockchain analytics | External service integration |
| Regulatory reporting automation | Jurisdiction-specific |
| Multi-jurisdiction rules | Complex rule inheritance |
| Real-time sanctions screening | External API dependency |

---

## 12. Success Criteria

- [ ] `rule!` macro compiles valid rules
- [ ] `rule_set!` macro creates evaluable rule sets
- [ ] `banking_scenario!` macro runs test scenarios
- [ ] AML hooks block transactions when rules trigger
- [ ] Compliance logs are immutable and queryable
- [ ] Built-in AML rules detect common patterns
- [ ] KYC limits enforced based on verification level
- [ ] CLI commands for compliance management
- [ ] 120+ tests passing
- [ ] Documentation for BA/Compliance team

---

## 13. Timeline Estimate

| Week | Focus | Deliverables |
|------|-------|--------------|
| W1-2 | DSL Foundation | `rule!`, `rule_set!` macros |
| W3 | Banking Scenarios | `banking_scenario!` macro |
| W4-5 | AML Hooks | Hook interface, registration |
| W6-7 | Compliance Engine | Engine, logging, reviews |
| W8 | Built-in Rules | AML standard ruleset |
| W9 | KYC Integration | Interface, limits |
| W10 | CLI & Testing | Commands, 120+ tests |

**Total:** ~10 weeks

---

## Appendix A: Example Rule Expansions

### A.1 Simple Rule

Input DSL:
```rust
rule!(
    name: "LARGE_TX",
    description: "Large transaction",
    when: transaction.amount >= 10_000 USDT,
    then: { flag_for_review("Large TX"); }
)
```

Expands to:
```rust
{
    pub struct LargeTxRule;

    impl Rule for LargeTxRule {
        fn name(&self) -> &'static str { "LARGE_TX" }
        fn description(&self) -> &'static str { "Large transaction" }
        fn hash(&self) -> &'static str { "sha256:abc123..." }

        fn evaluate(&self, ctx: &RuleContext) -> RuleResult {
            if ctx.transaction.amount >= rust_decimal::Decimal::new(10_000, 0) {
                RuleResult::Triggered {
                    actions: vec![
                        Action::FlagForReview("Large TX".into()),
                    ],
                }
            } else {
                RuleResult::Passed
            }
        }
    }

    LargeTxRule
}
```

### A.2 Complex Condition

Input DSL:
```rust
when: user.account_age < 7.days
  and transaction.intent == Withdrawal
  and transaction.amount >= 5_000 USDT
```

Expands to:
```rust
ctx.user.account_age < chrono::Duration::days(7)
    && ctx.transaction.intent == TransactionIntent::Withdrawal
    && ctx.transaction.amount >= rust_decimal::Decimal::new(5_000, 0)
```

---

## Appendix B: Compliance Report Sample

```
═══════════════════════════════════════════════════════════════════
                    BIBANK COMPLIANCE REPORT
                    Period: 2026-01-01 to 2026-01-31
═══════════════════════════════════════════════════════════════════

SUMMARY
───────────────────────────────────────────────────────────────────
Total Transactions Checked:     45,231
Approved:                       44,892 (99.25%)
Flagged for Review:                312 (0.69%)
Blocked:                            27 (0.06%)

RULE TRIGGERS
───────────────────────────────────────────────────────────────────
CTR_THRESHOLD:                     156
LARGE_TX_ALERT:                     89
RAPID_MOVEMENT:                     45
STRUCTURING_DETECTION:              23
NEW_ACCOUNT_LARGE_WD:               18
PEP_MONITORING:                      8

REVIEW OUTCOMES
───────────────────────────────────────────────────────────────────
Reviews Completed:                  298
Approved after Review:              271 (90.9%)
Rejected after Review:               27 (9.1%)
Pending Review:                      14

HIGH RISK USERS
───────────────────────────────────────────────────────────────────
User ID         Risk Score    Flags    Last Flag Date
USER-12345      HIGH          5        2026-01-28
USER-67890      HIGH          3        2026-01-25
USER-11111      MEDIUM        2        2026-01-22

═══════════════════════════════════════════════════════════════════
Report Generated: 2026-02-01 00:00:00 UTC
Report Hash: sha256:def456...
═══════════════════════════════════════════════════════════════════
```
