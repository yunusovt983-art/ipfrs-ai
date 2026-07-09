use thiserror::Error;

#[derive(Debug, Error)]
pub enum KError {
    #[error("core: {0}")]
    Core(#[from] ipfrs_core::Error),
    #[error("decode: {0}")]
    Decode(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("graph: {0}")]
    Graph(String),
}

pub type KResult<T> = Result<T, KError>;
