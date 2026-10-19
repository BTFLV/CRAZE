use thiserror::Error;

/// Errors carry user-facing English messages. The app translates them for its German UI
/// (see app/src/lib/messages.ts), so keep both in sync when changing a message.
#[derive(Debug, Error)]
pub enum CrazeError {
    #[error("GDS error: {0}")]
    Gds(String),
    #[error("{0}")]
    Setup(String),
    #[error("Solver error: {0}")]
    Solver(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Format error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CrazeError>;
