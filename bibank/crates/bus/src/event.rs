//! Ledger events for pub/sub distribution

use bibank_ledger::JournalEntry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the ledger system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LedgerEvent {
    /// A journal entry was committed
    EntryCommitted {
        /// The committed entry
        entry: JournalEntry,
        /// When the event was published
        timestamp: DateTime<Utc>,
    },

    /// Hash chain was verified
    ChainVerified {
        /// Last verified sequence number
        last_sequence: u64,
        /// Hash of the last entry
        last_hash: String,
    },

    /// Replay has started
    ReplayStarted {
        /// Starting sequence number
        from_sequence: u64,
    },

    /// Replay has completed
    ReplayCompleted {
        /// Number of entries replayed
        entries_count: usize,
    },
}

impl LedgerEvent {
    /// Create an EntryCommitted event
    pub fn entry_committed(entry: JournalEntry) -> Self {
        Self::EntryCommitted {
            entry,
            timestamp: Utc::now(),
        }
    }

    /// Create a ChainVerified event
    pub fn chain_verified(last_sequence: u64, last_hash: String) -> Self {
        Self::ChainVerified {
            last_sequence,
            last_hash,
        }
    }

    /// Create a ReplayStarted event
    pub fn replay_started(from_sequence: u64) -> Self {
        Self::ReplayStarted { from_sequence }
    }

    /// Create a ReplayCompleted event
    pub fn replay_completed(entries_count: usize) -> Self {
        Self::ReplayCompleted { entries_count }
    }
}
