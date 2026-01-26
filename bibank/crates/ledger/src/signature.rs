//! Digital signatures for journal entries
//!
//! Each entry is signed by the system key, and optionally by operator keys
//! for Adjustment entries requiring human approval.

use crate::entry::{JournalEntry, Posting, TransactionIntent};
use crate::error::LedgerError;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signature algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    /// Ed25519 (default)
    Ed25519,
    /// Secp256k1 (for future blockchain compatibility)
    Secp256k1,
}

impl Default for SignatureAlgorithm {
    fn default() -> Self {
        Self::Ed25519
    }
}

/// Digital signature attached to a journal entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySignature {
    /// Signer identifier ("SYSTEM" or operator ID)
    pub signer_id: String,

    /// Signature algorithm used
    pub algorithm: SignatureAlgorithm,

    /// Public key (hex-encoded)
    pub public_key: String,

    /// Signature bytes (hex-encoded)
    pub signature: String,

    /// Timestamp when signature was created
    pub signed_at: DateTime<Utc>,
}

impl EntrySignature {
    /// Verify this signature against a payload
    pub fn verify(&self, payload: &[u8]) -> Result<(), LedgerError> {
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => {
                let pk_bytes = hex::decode(&self.public_key).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key hex: {}", e),
                    }
                })?;

                let sig_bytes = hex::decode(&self.signature).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid signature hex: {}", e),
                    }
                })?;

                let pk_array: [u8; 32] = pk_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Public key must be 32 bytes".to_string(),
                    }
                })?;

                let sig_array: [u8; 64] = sig_bytes.try_into().map_err(|_| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: "Signature must be 64 bytes".to_string(),
                    }
                })?;

                let verifying_key = VerifyingKey::from_bytes(&pk_array).map_err(|e| {
                    LedgerError::InvalidSignature {
                        signer: self.signer_id.clone(),
                        reason: format!("Invalid public key: {}", e),
                    }
                })?;

                let signature = Signature::from_bytes(&sig_array);

                verifying_key.verify(payload, &signature).map_err(|e| {
                    LedgerError::SignatureVerificationFailed(format!(
                        "Signature from {} failed: {}",
                        self.signer_id, e
                    ))
                })?;

                Ok(())
            }
            SignatureAlgorithm::Secp256k1 => {
                // Future: implement secp256k1 verification
                Err(LedgerError::SignatureVerificationFailed(
                    "Secp256k1 not yet implemented".to_string(),
                ))
            }
        }
    }
}

/// Signable payload - the 8 fields that are signed
///
/// This is a deterministic representation of the entry for signing.
/// Order matters for consistent hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePayload {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub intent: TransactionIntent,
    pub postings: Vec<Posting>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub prev_hash: String,
    pub hash: String,
    pub signed_at: DateTime<Utc>,
}

impl SignablePayload {
    /// Create from a journal entry and signing timestamp
    pub fn from_entry(entry: &JournalEntry, signed_at: DateTime<Utc>) -> Self {
        Self {
            sequence: entry.sequence,
            timestamp: entry.timestamp,
            intent: entry.intent,
            postings: entry.postings.clone(),
            metadata: entry.metadata.clone(),
            prev_hash: entry.prev_hash.clone(),
            hash: entry.hash.clone(),
            signed_at,
        }
    }

    /// Serialize to canonical JSON bytes for signing
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SignablePayload serialization should never fail")
    }
}

/// Trait for signers
pub trait Signer: Send + Sync {
    /// Get the signer ID
    fn signer_id(&self) -> &str;

    /// Get the public key (hex-encoded)
    fn public_key_hex(&self) -> String;

    /// Sign a payload and return the signature
    fn sign(&self, entry: &JournalEntry) -> EntrySignature;
}

/// System signer using Ed25519
pub struct SystemSigner {
    signing_key: SigningKey,
}

impl SystemSigner {
    /// Create from a 32-byte seed (hex-encoded in env var)
    pub fn from_hex(hex_seed: &str) -> Result<Self, LedgerError> {
        let bytes = hex::decode(hex_seed).map_err(|e| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: format!("Invalid key hex: {}", e),
            }
        })?;

        let seed: [u8; 32] = bytes.try_into().map_err(|_| {
            LedgerError::InvalidSignature {
                signer: "SYSTEM".to_string(),
                reason: "Key must be 32 bytes".to_string(),
            }
        })?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    /// Generate a new random signing key
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            signing_key: SigningKey::generate(&mut rng),
        }
    }

    /// Export the seed as hex (for storage)
    pub fn seed_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }
}

impl Signer for SystemSigner {
    fn signer_id(&self) -> &str {
        "SYSTEM"
    }

    fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    fn sign(&self, entry: &JournalEntry) -> EntrySignature {
        let signed_at = Utc::now();
        let payload = SignablePayload::from_entry(entry, signed_at);
        let payload_bytes = payload.to_bytes();

        let signature = self.signing_key.sign(&payload_bytes);

        EntrySignature {
            signer_id: self.signer_id().to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: self.public_key_hex(),
            signature: hex::encode(signature.to_bytes()),
            signed_at,
        }
    }
}

impl JournalEntry {
    /// Verify all signatures on this entry
    pub fn verify_signatures(&self) -> Result<(), LedgerError> {
        if self.signatures.is_empty() {
            // Phase 1 entries have no signatures - that's OK
            return Ok(());
        }

        for sig in &self.signatures {
            let payload = SignablePayload::from_entry(self, sig.signed_at);
            sig.verify(&payload.to_bytes())?;
        }

        Ok(())
    }

    /// Check if entry has a system signature
    pub fn has_system_signature(&self) -> bool {
        self.signatures.iter().any(|s| s.signer_id == "SYSTEM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountKey;
    use bibank_core::Amount;
    use rust_decimal::Decimal;

    fn amount(val: i64) -> Amount {
        Amount::new(Decimal::new(val, 0)).unwrap()
    }

    fn make_test_entry() -> JournalEntry {
        JournalEntry {
            sequence: 1,
            prev_hash: "GENESIS".to_string(),
            hash: "abc123".to_string(),
            timestamp: Utc::now(),
            intent: TransactionIntent::Deposit,
            correlation_id: "test-1".to_string(),
            causality_id: None,
            postings: vec![
                Posting::debit(AccountKey::system_vault("USDT"), amount(100)),
                Posting::credit(AccountKey::user_available("ALICE", "USDT"), amount(100)),
            ],
            metadata: HashMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn test_system_signer_sign_and_verify() {
        let signer = SystemSigner::generate();
        let entry = make_test_entry();

        let signature = signer.sign(&entry);

        assert_eq!(signature.signer_id, "SYSTEM");
        assert_eq!(signature.algorithm, SignatureAlgorithm::Ed25519);

        // Verify the signature
        let payload = SignablePayload::from_entry(&entry, signature.signed_at);
        assert!(signature.verify(&payload.to_bytes()).is_ok());
    }

    #[test]
    fn test_signature_roundtrip() {
        let signer = SystemSigner::generate();
        let seed = signer.seed_hex();

        // Recreate signer from seed
        let signer2 = SystemSigner::from_hex(&seed).unwrap();
        assert_eq!(signer.public_key_hex(), signer2.public_key_hex());
    }

    #[test]
    fn test_entry_with_signature() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        assert!(entry.verify_signatures().is_ok());
        assert!(entry.has_system_signature());
    }

    #[test]
    fn test_tampered_entry_fails_verification() {
        let signer = SystemSigner::generate();
        let mut entry = make_test_entry();

        let signature = signer.sign(&entry);
        entry.signatures.push(signature);

        // Tamper with the entry
        entry.sequence = 999;

        // Verification should fail
        assert!(entry.verify_signatures().is_err());
    }
}
