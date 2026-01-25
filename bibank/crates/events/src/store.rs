//! JSONL event store - append-only writer

use crate::error::EventError;
use bibank_ledger::JournalEntry;
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Append-only JSONL event store
pub struct EventStore {
    base_path: PathBuf,
    current_file: Option<BufWriter<File>>,
    current_date: Option<String>,
}

impl EventStore {
    /// Create a new event store at the given path
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, EventError> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            current_file: None,
            current_date: None,
        })
    }

    /// Append a journal entry to the store
    pub fn append(&mut self, entry: &JournalEntry) -> Result<(), EventError> {
        let date = entry.timestamp.format("%Y-%m-%d").to_string();

        // Rotate file if date changed
        if self.current_date.as_ref() != Some(&date) {
            self.rotate_file(&date)?;
        }

        // Write entry as JSON line
        if let Some(ref mut writer) = self.current_file {
            let json = serde_json::to_string(entry)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
        }

        Ok(())
    }

    /// Rotate to a new file for the given date
    fn rotate_file(&mut self, date: &str) -> Result<(), EventError> {
        // Flush current file
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }

        // Open new file
        let file_path = self.base_path.join(format!("{}.jsonl", date));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        self.current_file = Some(BufWriter::new(file));
        self.current_date = Some(date.to_string());

        Ok(())
    }

    /// Get the path to today's file
    pub fn today_file_path(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.base_path.join(format!("{}.jsonl", date))
    }

    /// List all JSONL files in the store
    pub fn list_files(&self) -> Result<Vec<PathBuf>, EventError> {
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "jsonl") {
                files.push(path);
            }
        }

        files.sort();
        Ok(files)
    }

    /// Flush and close the current file
    pub fn close(&mut self) -> Result<(), EventError> {
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }
        self.current_file = None;
        self.current_date = None;
        Ok(())
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
