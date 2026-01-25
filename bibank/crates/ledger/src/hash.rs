//! Hash chain utilities for ledger integrity

use crate::entry::JournalEntry;
use sha2::{Digest, Sha256};

/// Calculate SHA256 hash of entry content (excluding the hash field itself)
pub fn calculate_entry_hash(entry: &JournalEntry) -> String {
    let mut hasher = Sha256::new();

    // Include all fields except `hash`
    hasher.update(entry.sequence.to_le_bytes());
    hasher.update(entry.prev_hash.as_bytes());
    hasher.update(entry.timestamp.to_rfc3339().as_bytes());
    hasher.update(format!("{:?}", entry.intent).as_bytes());
    hasher.update(entry.correlation_id.as_bytes());

    if let Some(ref causality_id) = entry.causality_id {
        hasher.update(causality_id.as_bytes());
    }

    // Hash postings
    for posting in &entry.postings {
        hasher.update(posting.account.to_string().as_bytes());
        hasher.update(posting.amount.value().to_string().as_bytes());
        hasher.update(format!("{:?}", posting.side).as_bytes());
    }

    // Hash metadata keys (sorted for determinism)
    let mut keys: Vec<_> = entry.metadata.keys().collect();
    keys.sort();
    for key in keys {
        hasher.update(key.as_bytes());
        if let Some(value) = entry.metadata.get(key) {
            hasher.update(value.to_string().as_bytes());
        }
    }

    hex::encode(hasher.finalize())
}

/// Verify hash chain integrity
pub fn verify_chain(entries: &[JournalEntry]) -> Result<(), ChainError> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut prev_hash = "GENESIS".to_string();

    for (i, entry) in entries.iter().enumerate() {
        // Verify prev_hash links correctly
        if entry.prev_hash != prev_hash {
            return Err(ChainError::BrokenLink {
                sequence: entry.sequence,
                expected: prev_hash,
                actual: entry.prev_hash.clone(),
            });
        }

        // Verify hash is correct
        let calculated = calculate_entry_hash(entry);
        if entry.hash != calculated {
            return Err(ChainError::InvalidHash {
                sequence: entry.sequence,
                expected: calculated,
                actual: entry.hash.clone(),
            });
        }

        // Verify sequence is strictly increasing
        if i > 0 && entry.sequence != entries[i - 1].sequence + 1 {
            return Err(ChainError::InvalidSequence {
                expected: entries[i - 1].sequence + 1,
                actual: entry.sequence,
            });
        }

        prev_hash = entry.hash.clone();
    }

    Ok(())
}

/// Errors in hash chain verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    BrokenLink {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidHash {
        sequence: u64,
        expected: String,
        actual: String,
    },
    InvalidSequence {
        expected: u64,
        actual: u64,
    },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::BrokenLink {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Broken link at seq {}: expected prev_hash '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidHash {
                sequence,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Invalid hash at seq {}: expected '{}', got '{}'",
                    sequence, expected, actual
                )
            }
            ChainError::InvalidSequence { expected, actual } => {
                write!(
                    f,
                    "Invalid sequence: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for ChainError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountKey, Posting, TransactionIntent};
    use bibank_core::Amount;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    fn create_entry(sequence: u64, prev_hash: &str) -> JournalEntry {
        let amount = Amount::new(Decimal::new(100, 0)).unwrap();
        let mut entry = JournalEntry {
            sequence,
            prev_hash: prev_hash.to_string(),
            hash: String::new(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: format!("test-{}", sequence),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount),
            ],
            metadata: HashMap::new(),
        };
        entry.hash = calculate_entry_hash(&entry);
        entry
    }

    #[test]
    fn test_hash_deterministic() {
        let entry = create_entry(1, "GENESIS");
        let hash1 = calculate_entry_hash(&entry);
        let hash2 = calculate_entry_hash(&entry);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_verify_valid_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, &entry1.hash);
        let entry3 = create_entry(3, &entry2.hash);

        let entries = vec![entry1, entry2, entry3];
        assert!(verify_chain(&entries).is_ok());
    }

    #[test]
    fn test_verify_broken_chain() {
        let entry1 = create_entry(1, "GENESIS");
        let entry2 = create_entry(2, "wrong_hash");

        let entries = vec![entry1, entry2];
        let result = verify_chain(&entries);
        assert!(matches!(result, Err(ChainError::BrokenLink { .. })));
    }
}
