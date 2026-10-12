use thiserror::Error;

/// Errors carry user-facing (German) messages; they are shown directly in the UI.
#[derive(Debug, Error)]
pub enum CrazeError {
    #[error("GDS-Fehler: {0}")]
    Gds(String),
    #[error("{0}")]
    Setup(String),
    #[error("Solver-Fehler: {0}")]
    Solver(String),
    #[error("Abgebrochen")]
    Cancelled,
    #[error("Dateifehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("Formatfehler: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CrazeError>;
