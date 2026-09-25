use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{}: {}", .path.display(), .source)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path outside allowed roots: {}", .0.display())]
    OutsideRoots(PathBuf),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    System(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn io_err(path: &Path, source: std::io::Error) -> CoreError {
    CoreError::Io { path: path.to_path_buf(), source }
}
