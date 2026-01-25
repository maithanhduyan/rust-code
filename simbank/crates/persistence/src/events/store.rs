//! JSONL Event Store - append-only writer
//!
//! Ghi events vào files JSONL theo ngày để phục vụ AML audit trail.

use crate::error::PersistenceResult;
use chrono::Utc;
use simbank_core::Event;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Event Store - ghi events vào files JSONL.
///
/// Files được tổ chức theo ngày: `data/events/2026-01-25.jsonl`
pub struct EventStore {
    /// Thư mục chứa event files
    base_path: PathBuf,
    /// Counter cho event ID
    event_counter: AtomicU64,
    /// Current file writer (thread-safe)
    current_writer: Mutex<Option<EventWriter>>,
}

struct EventWriter {
    date: String,
    writer: BufWriter<File>,
}

impl EventStore {
    /// Tạo EventStore mới
    ///
    /// # Arguments
    /// * `base_path` - Đường dẫn thư mục chứa events (e.g., "data/events")
    pub fn new<P: AsRef<Path>>(base_path: P) -> PersistenceResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Tạo thư mục nếu chưa có
        fs::create_dir_all(&base_path)?;

        // Đọc event counter từ existing files
        let event_counter = Self::load_event_counter(&base_path)?;

        Ok(Self {
            base_path,
            event_counter: AtomicU64::new(event_counter),
            current_writer: Mutex::new(None),
        })
    }

    /// Lấy base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Load event counter từ files hiện có
    fn load_event_counter(base_path: &Path) -> PersistenceResult<u64> {
        let mut max_id: u64 = 0;

        if let Ok(entries) = fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "jsonl") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for line in content.lines() {
                            if let Ok(event) = serde_json::from_str::<Event>(line) {
                                // Parse event ID: EVT_000123 -> 123
                                if let Some(num_str) = event.event_id.strip_prefix("EVT_") {
                                    if let Ok(num) = num_str.parse::<u64>() {
                                        max_id = max_id.max(num);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(max_id + 1)
    }

    /// Lấy file path cho ngày hiện tại
    fn get_file_path(&self, date: &str) -> PathBuf {
        self.base_path.join(format!("{}.jsonl", date))
    }

    /// Lấy ngày hiện tại dạng string
    fn current_date() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    /// Generate event ID mới
    pub fn next_event_id(&self) -> String {
        let id = self.event_counter.fetch_add(1, Ordering::SeqCst);
        format!("EVT_{:06}", id)
    }

    /// Ghi event vào store
    pub fn append(&self, event: &Event) -> PersistenceResult<()> {
        let date = Self::current_date();
        let json = serde_json::to_string(event)?;

        let mut guard = self.current_writer.lock().unwrap();

        // Kiểm tra cần tạo file mới không
        let needs_new_file = guard
            .as_ref()
            .map_or(true, |w| w.date != date);

        if needs_new_file {
            let path = self.get_file_path(&date);
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            let writer = BufWriter::new(file);
            *guard = Some(EventWriter {
                date: date.clone(),
                writer,
            });
        }

        // Ghi event
        if let Some(ref mut w) = *guard {
            writeln!(w.writer, "{}", json)?;
            w.writer.flush()?;
        }

        Ok(())
    }

    /// Ghi nhiều events
    pub fn append_batch(&self, events: &[Event]) -> PersistenceResult<()> {
        for event in events {
            self.append(event)?;
        }
        Ok(())
    }

    /// Lấy tất cả event files
    pub fn list_files(&self) -> PersistenceResult<Vec<PathBuf>> {
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

    /// Lấy file path theo ngày
    pub fn get_file_for_date(&self, date: &str) -> Option<PathBuf> {
        let path = self.get_file_path(date);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Flush tất cả pending writes
    pub fn flush(&self) -> PersistenceResult<()> {
        let mut guard = self.current_writer.lock().unwrap();
        if let Some(ref mut w) = *guard {
            w.writer.flush()?;
        }
        Ok(())
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    #[allow(unused_imports)]
    use simbank_core::PersonType;
    use tempfile::tempdir;

    #[test]
    fn test_event_store_append() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        let event_id = store.next_event_id();
        let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(100), "USDT");

        store.append(&event).unwrap();
        store.flush().unwrap();

        // Verify file exists
        let files = store.list_files().unwrap();
        assert_eq!(files.len(), 1);

        // Verify content
        let content = fs::read_to_string(&files[0]).unwrap();
        assert!(content.contains("EVT_000001"));
        assert!(content.contains("deposit"));
    }

    #[test]
    fn test_event_store_counter() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path()).unwrap();

        assert_eq!(store.next_event_id(), "EVT_000001");
        assert_eq!(store.next_event_id(), "EVT_000002");
        assert_eq!(store.next_event_id(), "EVT_000003");
    }

    #[test]
    fn test_event_store_reload_counter() {
        let dir = tempdir().unwrap();

        // First store
        {
            let store = EventStore::new(dir.path()).unwrap();
            let event_id = store.next_event_id();
            let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(100), "USDT");
            store.append(&event).unwrap();

            let event_id = store.next_event_id();
            let event = Event::deposit(&event_id, "CUST_001", "ACC_001", dec!(200), "USDT");
            store.append(&event).unwrap();
        }

        // Second store - should continue from 3
        {
            let store = EventStore::new(dir.path()).unwrap();
            assert_eq!(store.next_event_id(), "EVT_000003");
        }
    }
}