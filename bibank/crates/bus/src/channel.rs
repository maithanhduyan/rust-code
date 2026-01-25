//! Simple in-process event bus
//!
//! Phase 1: Synchronous callback-based distribution
//! Phase 2+: Async channels with replay

use bibank_events::EventReader;
use std::path::Path;

/// Event bus for distributing committed events
///
/// Phase 1 implementation is minimal - just wraps EventReader for replay
pub struct EventBus {
    journal_path: std::path::PathBuf,
}

impl EventBus {
    /// Create a new event bus
    pub fn new(journal_path: impl AsRef<Path>) -> Self {
        Self {
            journal_path: journal_path.as_ref().to_path_buf(),
        }
    }

    /// Get an event reader for replay
    pub fn reader(&self) -> Result<EventReader, bibank_events::EventError> {
        EventReader::from_directory(&self.journal_path)
    }

    /// Get the journal path
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }
}
