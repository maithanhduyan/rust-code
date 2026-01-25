//! Projection errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Event error: {0}")]
    Event(#[from] bibank_events::EventError),

    #[error("Projection not initialized")]
    NotInitialized,
}
