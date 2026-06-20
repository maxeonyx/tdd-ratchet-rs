// Status file: tracks per-test expected states in .test-status.json

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedStatus {
    pub tests: BTreeMap<String, TestState>,
}

impl TrackedStatus {
    pub fn new(tests: BTreeMap<String, TestState>) -> Self {
        Self { tests }
    }

    pub fn empty() -> Self {
        Self::new(BTreeMap::new())
    }

    pub fn set_test_state(&mut self, test_name: impl Into<String>, state: TestState) {
        self.tests.insert(test_name.into(), state);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingTreeInstructions {
    pub renames: BTreeMap<String, String>,
    pub removals: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusFile {
    /// JSON Schema reference — always set to the canonical URL on save.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    schema: Option<String>,

    /// Immutable adoption baseline. The commit at which this project began
    /// enforcing the ratchet. Once set, it must never change (see the
    /// two-commit immutability check). Absent on projects that adopted before
    /// this field existed, and on fresh projects until they deliberately set it.
    #[serde(
        default,
        rename = "rewrite_red_baseline",
        skip_serializing_if = "Option::is_none"
    )]
    pub baseline: Option<String>,

    pub tests: BTreeMap<String, TestState>,

    /// Non-test checks (e.g. clippy/fmt) tracked alongside tests. Carried
    /// through parse/serialize verbatim. tdd-ratchet does not interpret or
    /// enforce these; the field exists so files that use it are not rejected.
    #[serde(
        default,
        rename = "rewrite_red_checks",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub checks: BTreeMap<String, TestState>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub renames: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removals: BTreeSet<String>,
}

/// Lenient per-test entry used only on the historical-read path. Accepts the
/// legacy object form {state, baseline} and the bare string form, keeps state,
/// discards baseline. The strict path (parse_from_str) does not use this.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum HistoricalTestEntry {
    Simple(TestState),
    // `baseline: Option<String>` (not required) so an object form that
    // somehow lacks baseline still parses; we only care about `state`.
    WithBaseline {
        state: TestState,
        #[allow(dead_code)]
        baseline: Option<String>,
    },
}

impl HistoricalTestEntry {
    fn state(&self) -> TestState {
        match self {
            HistoricalTestEntry::Simple(s) => *s,
            HistoricalTestEntry::WithBaseline { state, .. } => *state,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HistoricalStatusFile {
    #[serde(rename = "$schema", default)]
    schema: Option<String>,
    #[serde(default)]
    baseline: Option<String>,
    tests: BTreeMap<String, HistoricalTestEntry>,
    #[serde(default)]
    renames: BTreeMap<String, String>,
    // No deny_unknown_fields here → top-level unknowns still tolerated.
}

impl StatusFile {
    pub fn new(tests: BTreeMap<String, TestState>) -> Self {
        StatusFile::from_parts(
            TrackedStatus::new(tests),
            WorkingTreeInstructions::default(),
        )
    }

    pub fn from_parts(status: TrackedStatus, instructions: WorkingTreeInstructions) -> Self {
        StatusFile {
            schema: None,
            baseline: None,
            tests: status.tests,
            checks: BTreeMap::new(),
            renames: instructions.renames,
            removals: BTreeSet::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new(BTreeMap::new())
    }

    pub fn tracked_status(&self) -> TrackedStatus {
        TrackedStatus {
            tests: self.tests.clone(),
        }
    }

    pub fn into_tracked_status(self) -> TrackedStatus {
        TrackedStatus { tests: self.tests }
    }

    pub fn working_tree_instructions(&self) -> WorkingTreeInstructions {
        WorkingTreeInstructions {
            renames: self.renames.clone(),
            removals: self.removals.clone(),
        }
    }

    pub fn set_test_state(&mut self, test_name: impl Into<String>, state: TestState) {
        let mut tracked = self.tracked_status();
        tracked.set_test_state(test_name, state);
        self.tests = tracked.tests;
    }

    pub fn read_from_path(path: &Path) -> Result<Self, StatusFileError> {
        let contents = std::fs::read_to_string(path).map_err(|e| StatusFileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::parse_from_str(&contents, path)
    }

    pub fn write_to_path(&self, path: &Path) -> Result<(), StatusFileError> {
        // Always write the $schema key. Working-tree removals are transient and
        // never persisted into the ratchet-generated output.
        let mut with_schema = self.clone();
        with_schema.schema = Some(SCHEMA_URL.to_string());
        with_schema.removals.clear();
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

    pub fn parse_from_str(contents: &str, path: &Path) -> Result<Self, StatusFileError> {
        serde_json::from_str(contents).map_err(|e| StatusFileError::Parse {
            path: path.to_path_buf(),
            source: e,
        })
    }

    pub fn parse_historical_from_str(contents: &str, path: &Path) -> Result<Self, StatusFileError> {
        let historical: HistoricalStatusFile =
            serde_json::from_str(contents).map_err(|e| StatusFileError::Parse {
                path: path.to_path_buf(),
                source: e,
            })?;

        Ok(StatusFile {
            schema: historical.schema,
            baseline: historical.baseline,
            tests: historical
                .tests
                .into_iter()
                .map(|(k, v)| (k, v.state()))
                .collect(),
            checks: BTreeMap::new(),
            renames: historical.renames,
            removals: BTreeSet::new(),
        })
    }

    pub fn load(path: &Path) -> Result<Self, StatusFileError> {
        Self::read_from_path(path)
    }

    pub fn save(&self, path: &Path) -> Result<(), StatusFileError> {
        self.write_to_path(path)
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
