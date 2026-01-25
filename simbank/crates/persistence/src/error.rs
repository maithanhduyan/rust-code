//! # Persistence Errors
//!
//! Error types cho persistence layer, wrapping sqlx và IO errors.

use thiserror::Error;

/// Persistence layer errors
#[derive(Debug, Error)]
pub enum PersistenceError {
    // === Database errors ===
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Record not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Record already exists: {entity} with id {id}")]
    AlreadyExists { entity: String, id: String },

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    // === Event store errors ===
    #[error("Event store IO error: {0}")]
    EventStoreIo(#[from] std::io::Error),

    #[error("Event serialization error: {0}")]
    EventSerialization(#[from] serde_json::Error),

    #[error("Event file not found: {0}")]
    EventFileNotFound(String),

    // === Conversion errors ===
    #[error("Invalid decimal value: {0}")]
    InvalidDecimal(String),

    #[error("Invalid enum value: {field} = {value}")]
    InvalidEnumValue { field: String, value: String },

    // === Configuration errors ===
    #[error("Configuration error: {0}")]
    Configuration(String),

    // === Other errors ===
    #[error("{0}")]
    Other(String),
}

/// Result type alias cho PersistenceError
pub type PersistenceResult<T> = Result<T, PersistenceError>;

impl PersistenceError {
    /// Tạo NotFound error
    pub fn not_found(entity: &str, id: &str) -> Self {
        Self::NotFound {
            entity: entity.to_string(),
            id: id.to_string(),
        }
    }

    /// Tạo AlreadyExists error
    pub fn already_exists(entity: &str, id: &str) -> Self {
        Self::AlreadyExists {
            entity: entity.to_string(),
            id: id.to_string(),
        }
    }

    /// Kiểm tra có phải lỗi not found không
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    /// Kiểm tra có phải lỗi database không
    pub fn is_database_error(&self) -> bool {
        matches!(self, Self::Database(_))
    }
}
