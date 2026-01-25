//! JSONL event reader - sequential reader for replay

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Sequential event reader for replay
pub struct EventReader {
    files: Vec<std::path::PathBuf>,
}

impl EventReader {
    /// Create a new reader from a directory
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, EventError> {
        let path = path.as_ref();
        let mut files = Vec::new();

        if path.exists() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.extension().map_or(false, |ext| ext == "jsonl") {
                    files.push(file_path);
                }
            }
        }

        files.sort();

        Ok(Self { files })
    }

    /// Read all entries from all files in order
    pub fn read_all(&self) -> Result<Vec<JournalEntry>, EventError> {
        let mut entries = Vec::new();

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry: JournalEntry = serde_json::from_str(&line)?;
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Get the last sequence number from all files
    pub fn last_sequence(&self) -> Result<Option<u64>, EventError> {
        if self.files.is_empty() {
            return Ok(None);
        }

        // Read from the last file
        let last_file = &self.files[self.files.len() - 1];
        let file = File::open(last_file)?;
        let reader = BufReader::new(file);

        let mut last_seq = None;
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(&line)?;
            last_seq = Some(entry.sequence);
        }

        Ok(last_seq)
    }

    /// Get the last entry (for prev_hash)
    pub fn last_entry(&self) -> Result<Option<JournalEntry>, EventError> {
        let entries = self.read_all()?;
        Ok(entries.into_iter().last())
    }

    /// Count total entries across all files
    pub fn count(&self) -> Result<usize, EventError> {
        let mut count = 0;

        for file_path in &self.files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if !line.trim().is_empty() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}
