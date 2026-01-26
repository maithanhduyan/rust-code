//! Compliance Ledger - Append-only JSONL storage
//!
//! This is the "Decision Truth" ledger, separate from the main financial ledger.
//! All writes are append-only and immutable.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::error::ComplianceResult;
use crate::event::ComplianceEvent;

/// Append-only JSONL ledger for compliance events
///
/// Each line is a JSON-serialized ComplianceEvent.
/// The file is append-only and should never be modified.
pub struct ComplianceLedger {
    path: PathBuf,
    file: Option<File>,
}

impl ComplianceLedger {
    /// Create a new ledger at the given path
    pub fn new(path: impl AsRef<Path>) -> ComplianceResult<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            path,
            file: Some(file),
        })
    }

    /// Create an in-memory ledger (for testing)
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            file: None,
        }
    }

    /// Append an event to the ledger
    pub fn append(&mut self, event: &ComplianceEvent) -> ComplianceResult<()> {
        if let Some(ref mut file) = self.file {
            let json = serde_json::to_string(event)?;
            writeln!(file, "{}", json)?;
            file.flush()?;
            Ok(())
        } else {
            // In-memory mode - just validate serialization
            let _ = serde_json::to_string(event)?;
            Ok(())
        }
    }

    /// Read all events from the ledger
    pub fn read_all(&self) -> ComplianceResult<Vec<ComplianceEvent>> {
        if self.file.is_none() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: ComplianceEvent = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Read events from a specific point (by line number)
    pub fn read_from(&self, start_line: usize) -> ComplianceResult<Vec<ComplianceEvent>> {
        if self.file.is_none() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            if i < start_line {
                continue;
            }
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: ComplianceEvent = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get the current line count (for checkpointing)
    pub fn line_count(&self) -> ComplianceResult<usize> {
        if self.file.is_none() {
            return Ok(0);
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count())
    }

    /// Get the path to the ledger file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if this is an in-memory ledger
    pub fn is_in_memory(&self) -> bool {
        self.file.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::AmlDecision;
    use tempfile::tempdir;

    #[test]
    fn test_in_memory_ledger() {
        let mut ledger = ComplianceLedger::in_memory();

        let event = ComplianceEvent::check_performed(
            "TX-001",
            "USER-001",
            AmlDecision::Approved,
            vec![],
        );

        ledger.append(&event).unwrap();

        assert!(ledger.is_in_memory());
        assert_eq!(ledger.read_all().unwrap().len(), 0); // In-memory doesn't store
    }

    #[test]
    fn test_file_ledger_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        let event1 = ComplianceEvent::check_performed(
            "TX-001",
            "USER-001",
            AmlDecision::Approved,
            vec!["RULE_1".to_string()],
        );
        let event2 = ComplianceEvent::check_performed(
            "TX-002",
            "USER-002",
            AmlDecision::blocked("Sanctions", "OFAC-001"),
            vec!["WATCHLIST".to_string()],
        );

        // Write events
        {
            let mut ledger = ComplianceLedger::new(&path).unwrap();
            ledger.append(&event1).unwrap();
            ledger.append(&event2).unwrap();
        }

        // Read events
        {
            let ledger = ComplianceLedger::new(&path).unwrap();
            let events = ledger.read_all().unwrap();

            assert_eq!(events.len(), 2);
            assert_eq!(events[0].id(), event1.id());
            assert_eq!(events[1].id(), event2.id());
        }
    }

    #[test]
    fn test_line_count() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        let mut ledger = ComplianceLedger::new(&path).unwrap();

        assert_eq!(ledger.line_count().unwrap(), 0);

        ledger
            .append(&ComplianceEvent::check_performed(
                "TX-001",
                "USER-001",
                AmlDecision::Approved,
                vec![],
            ))
            .unwrap();

        // Need to re-read to get updated count
        let ledger = ComplianceLedger::new(&path).unwrap();
        assert_eq!(ledger.line_count().unwrap(), 1);
    }

    #[test]
    fn test_read_from_offset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("compliance.jsonl");

        // Write 5 events
        {
            let mut ledger = ComplianceLedger::new(&path).unwrap();
            for i in 0..5 {
                ledger
                    .append(&ComplianceEvent::check_performed(
                        format!("TX-{:03}", i),
                        "USER-001",
                        AmlDecision::Approved,
                        vec![],
                    ))
                    .unwrap();
            }
        }

        // Read from offset 3
        {
            let ledger = ComplianceLedger::new(&path).unwrap();
            let events = ledger.read_from(3).unwrap();

            assert_eq!(events.len(), 2);
        }
    }

    #[test]
    fn test_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("compliance.jsonl");

        let ledger = ComplianceLedger::new(&path).unwrap();
        assert!(!ledger.is_in_memory());
        assert!(path.parent().unwrap().exists());
    }
}
