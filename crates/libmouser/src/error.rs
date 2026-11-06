//! Crate-wide error type.

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    PlainIo(#[from] std::io::Error),

    /// A `.device` file could not be understood.
    #[error("{path}:{line}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },

    /// Nothing in the database matched this hardware. The usual outcome for
    /// most hidraw nodes on a system — callers generally skip these quietly.
    #[error("no device database entry for {0}")]
    UnknownDevice(String),

    /// The database named a driver this build does not have.
    #[error("unknown driver `{0}`")]
    UnknownDriver(String),

    /// The driver saw the hardware and declined it.
    #[error("driver `{driver}` rejected the device: {message}")]
    ProbeFailed { driver: String, message: String },

    #[error("no such {kind} (index {index})")]
    NoSuchIndex { kind: &'static str, index: u32 },

    /// The hardware cannot do what was asked of it.
    #[error("device does not support {0}")]
    Unsupported(&'static str),

    /// A value fell outside the set the device accepts.
    #[error("invalid value for {field}: {message}")]
    InvalidValue {
        field: &'static str,
        message: String,
    },

    /// Writing the staged configuration to the device failed.
    #[error("failed to write configuration to device: {0}")]
    Commit(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub fn parse(path: impl Into<PathBuf>, line: usize, message: impl Into<String>) -> Self {
        Error::Parse {
            path: path.into(),
            line,
            message: message.into(),
        }
    }

    pub fn invalid(field: &'static str, message: impl Into<String>) -> Self {
        Error::InvalidValue {
            field,
            message: message.into(),
        }
    }
}
