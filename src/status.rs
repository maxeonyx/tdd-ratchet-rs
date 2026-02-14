// Status file: tracks per-test expected states in .test-status.json

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestState {
    Pending,
    Passing,
}

impl fmt::Display for TestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestState::Pending => write!(f, "pending"),
            TestState::Passing => write!(f, "passing"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusFile {
    pub tests: BTreeMap<String, TestState>,
}

impl StatusFile {
    pub fn empty() -> Self {
        StatusFile {
            tests: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, StatusFileError> {
        let contents = std::fs::read_to_string(path).map_err(|e| StatusFileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let status: StatusFile =
            serde_json::from_str(&contents).map_err(|e| StatusFileError::Parse {
                path: path.to_path_buf(),
                source: e,
            })?;
        Ok(status)
    }

    pub fn save(&self, path: &Path) -> Result<(), StatusFileError> {
        let contents =
            serde_json::to_string_pretty(self).map_err(|e| StatusFileError::Serialize {
                path: path.to_path_buf(),
                source: e,
            })?;
        std::fs::write(path, contents + "\n").map_err(|e| StatusFileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum StatusFileError {
    Io {
        path: std::path::PathBuf,
        source: io::Error,
    },
    Parse {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    Serialize {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for StatusFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusFileError::Io { path, source } => {
                write!(
                    f,
                    "Failed to read/write status file {}: {}",
                    path.display(),
                    source
                )
            }
            StatusFileError::Parse { path, source } => {
                write!(
                    f,
                    "Failed to parse JSON in status file {}: {}",
                    path.display(),
                    source
                )
            }
            StatusFileError::Serialize { path, source } => {
                write!(
                    f,
                    "Failed to serialize status file {}: {}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl std::error::Error for StatusFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StatusFileError::Io { source, .. } => Some(source),
            StatusFileError::Parse { source, .. } => Some(source),
            StatusFileError::Serialize { source, .. } => Some(source),
        }
    }
}
