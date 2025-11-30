use thiserror::Error;

/// Error variants surfaced by Bento shader compilation and inspection routines.
#[derive(Debug, Error)]
pub enum BentoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Binary serialization error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Shader compilation error: {0}")]
    ShaderCompilation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Shader backend error: {0}")]
    Dashi(#[from] dashi::GPUError),
}
