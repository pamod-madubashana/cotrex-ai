use std::fmt;

#[derive(Debug)]
pub enum ModelManagerError {
    NotFound(String),
    AlreadyInstalled(String),
    DownloadFailed(String),
    ChecksumMismatch { expected: String, actual: String },
    Registry(String),
    Storage(String),
    Io(std::io::Error),
}

impl fmt::Display for ModelManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "model not found: {id}"),
            Self::AlreadyInstalled(id) => write!(f, "model already installed: {id}"),
            Self::DownloadFailed(msg) => write!(f, "download failed: {msg}"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(f, "checksum mismatch: expected {expected}, got {actual}")
            }
            Self::Registry(msg) => write!(f, "registry error: {msg}"),
            Self::Storage(msg) => write!(f, "storage error: {msg}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ModelManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ModelManagerError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
