// Status file: tracks per-test expected states in .test-status.json

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

pub const SCHEMA_URL: &str = "https://tdd-ratchet.maxeonyx.com/schema/test-status.v1.json";

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
#[serde(deny_unknown_fields)]
pub struct StatusFile {
    /// JSON Schema reference — always set to the canonical URL on save.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    pub tests: BTreeMap<String, TestState>,
    /// The commit hash at which the ratchet was initialized.
    /// Tests at or before this commit are grandfathered for history checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
}

impl StatusFile {
    pub fn new(tests: BTreeMap<String, TestState>, baseline: Option<String>) -> Self {
        StatusFile {
            schema: None,
            tests,
            baseline,
        }
    }

    pub fn empty() -> Self {
        Self::new(BTreeMap::new(), None)
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
        // Always write the $schema key
        let mut with_schema = self.clone();
        with_schema.schema = Some(SCHEMA_URL.to_string());
        let contents =
            serde_json::to_string_pretty(&with_schema).map_err(|e| StatusFileError::Serialize {
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
