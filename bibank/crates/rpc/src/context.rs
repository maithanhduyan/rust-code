//! Application context - wires everything together

use bibank_bus::EventBus;
use bibank_events::{EventReader, EventStore};
use bibank_ledger::{hash::calculate_entry_hash, JournalEntry, Signer, SystemSigner, UnsignedEntry};
use bibank_projection::ProjectionEngine;
use bibank_risk::{RiskEngine, RiskError};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Application context - wires together all components
pub struct AppContext {
    pub risk: RiskEngine,
    pub event_store: EventStore,
    pub bus: EventBus,
    pub projection: Option<ProjectionEngine>,
    pub signer: Option<Arc<dyn Signer>>,
    journal_path: PathBuf,
    projection_path: PathBuf,
    last_sequence: u64,
    last_hash: String,
}

impl AppContext {
    /// Create a new application context
    pub async fn new(data_path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let data_path = data_path.as_ref();
        let journal_path = data_path.join("journal");
        let projection_path = data_path.join("projection.db");

        // Create directories
        std::fs::create_dir_all(&journal_path)?;

        // Initialize components
        let event_store = EventStore::new(&journal_path)?;
        let bus = EventBus::new(&journal_path);
        let mut risk = RiskEngine::new();

        // Replay events to rebuild state
        let reader = EventReader::from_directory(&journal_path)?;
        let entries = reader.read_all()?;

        let (last_sequence, last_hash) = if let Some(last) = entries.last() {
            (last.sequence, last.hash.clone())
        } else {
            (0, "GENESIS".to_string())
        };

        // Rebuild risk state from events
        risk.replay(entries.iter());

        // Initialize projection
        let projection = ProjectionEngine::new(&projection_path).await.ok();

        // Replay projection if available
        if let Some(ref proj) = projection {
            proj.replay(&bus).await.ok();
        }

        // Initialize system signer from env var (Phase 2)
        let signer: Option<Arc<dyn Signer>> = std::env::var("BIBANK_SYSTEM_KEY")
            .ok()
            .and_then(|key| SystemSigner::from_hex(&key).ok())
            .map(|s| Arc::new(s) as Arc<dyn Signer>);

        Ok(Self {
            risk,
            event_store,
            bus,
            projection,
            signer,
            journal_path,
            projection_path,
            last_sequence,
            last_hash,
        })
    }

    /// Commit an unsigned entry
    ///
    /// Flow: Risk Check → Sign → Append → Apply
    pub async fn commit(&mut self, unsigned: UnsignedEntry) -> Result<JournalEntry, CommitError> {
        // 1. Validate double-entry balance
        unsigned.validate_balance().map_err(CommitError::Ledger)?;

        // 2. Risk check (pre-commit gatekeeper)
        self.risk.check(&unsigned).map_err(CommitError::Risk)?;

        // 3. Sign the entry (add sequence, prev_hash, hash, timestamp)
        let sequence = self.last_sequence + 1;
        let prev_hash = self.last_hash.clone();
        let timestamp = Utc::now();

        let mut entry = JournalEntry {
            sequence,
            prev_hash,
            hash: String::new(),
            timestamp,
            intent: unsigned.intent,
            correlation_id: unsigned.correlation_id,
            causality_id: unsigned.causality_id,
            postings: unsigned.postings,
            metadata: unsigned.metadata,
            signatures: Vec::new(), // Phase 2: Will be signed after hash calculation
        };

        entry.hash = calculate_entry_hash(&entry);

        // 4. Sign the entry with system key (Phase 2)
        if let Some(ref signer) = self.signer {
            let signature = signer.sign(&entry);
            entry.signatures.push(signature);
        }

        // 5. Validate the signed entry
        entry.validate().map_err(CommitError::Ledger)?;

        // 6. Append to event store (Source of Truth)
        self.event_store
            .append(&entry)
            .map_err(CommitError::Event)?;

        // 7. Update risk state
        self.risk.apply(&entry);

        // 8. Update projection (if available)
        if let Some(ref projection) = self.projection {
            projection.apply(&entry).await.ok();
        }

        // 9. Update last sequence/hash
        self.last_sequence = entry.sequence;
        self.last_hash = entry.hash.clone();

        Ok(entry)
    }

    /// Get journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Get projection path
    pub fn projection_path(&self) -> &Path {
        &self.projection_path
    }

    /// Check if system is initialized (has Genesis entry)
    pub fn is_initialized(&self) -> bool {
        self.last_sequence > 0
    }

    /// Get last sequence number
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
}

/// Errors during commit
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("Ledger error: {0}")]
    Ledger(#[from] bibank_ledger::LedgerError),

    #[error("Risk error: {0}")]
    Risk(RiskError),

    #[error("Event store error: {0}")]
    Event(#[from] bibank_events::EventError),
}
