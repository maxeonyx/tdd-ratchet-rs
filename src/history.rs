// Git history inspection: verify no test skipped the pending state.

use crate::ratchet::GATEKEEPER_TEST_NAME;
use crate::status::StatusFile;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum HistoryViolation {
    /// A test appeared as passing without ever being pending.
    SkippedPending { test: String, commit: String },
}

/// A snapshot of the status file at a specific commit.
#[derive(Debug, Clone)]
pub struct HistorySnapshot {
    pub commit: String,
    pub status: StatusFile,
}

/// Collect status file snapshots from git history.
///
/// Returns snapshots from oldest to newest, starting from the baseline
/// (inclusive) if set, or from the beginning of history.
pub fn collect_history_snapshots(
    repo_path: &Path,
    baseline: Option<&str>,
) -> Result<Vec<HistorySnapshot>, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;

    let mut snapshots = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

    let baseline_oid = baseline.map(|b| git2::Oid::from_str(b)).transpose()?;
    let mut past_baseline = baseline_oid.is_none();

    for oid_result in revwalk {
        let oid = oid_result?;

        if !past_baseline {
            if Some(oid) == baseline_oid {
                if let Some(sf) = status_file_at_commit(&repo, oid)? {
                    snapshots.push(HistorySnapshot {
                        commit: oid.to_string(),
                        status: sf,
                    });
                }
                past_baseline = true;
            }
            continue;
        }

        if let Some(sf) = status_file_at_commit(&repo, oid)? {
            snapshots.push(HistorySnapshot {
                commit: oid.to_string(),
                status: sf,
            });
        }
    }

    Ok(snapshots)
}

/// Check history snapshots for TDD violations. Pure function — no IO.
///
/// Verifies that every test that appears as "passing" had a prior
/// appearance as "pending". Tests in the first snapshot (when a baseline
/// is configured) are grandfathered. The gatekeeper test is always exempt.
pub fn check_history_snapshots(
    snapshots: &[HistorySnapshot],
    has_baseline: bool,
) -> Vec<HistoryViolation> {
    let mut first_seen: BTreeMap<String, (String, crate::status::TestState)> = BTreeMap::new();
    let mut violations = Vec::new();

    let first_snapshot_commit = if has_baseline {
        snapshots.first().map(|s| s.commit.clone())
    } else {
        None
    };

    for snapshot in snapshots {
        for (test_name, state) in &snapshot.status.tests {
            if !first_seen.contains_key(test_name) {
                first_seen.insert(test_name.clone(), (snapshot.commit.clone(), *state));
                if *state == crate::status::TestState::Passing {
                    let is_grandfathered = first_snapshot_commit
                        .as_ref()
                        .is_some_and(|first| &snapshot.commit == first);
                    let is_gatekeeper = test_name.ends_with(GATEKEEPER_TEST_NAME);
                    if !is_grandfathered && !is_gatekeeper {
                        violations.push(HistoryViolation::SkippedPending {
                            test: test_name.clone(),
                            commit: snapshot.commit.clone(),
                        });
                    }
                }
            }
        }
    }

    violations
}

/// Convenience: collect snapshots and check them in one call.
/// Used by existing callers that don't need the split.
pub fn check_history(
    repo_path: &Path,
    baseline: Option<&str>,
) -> Result<Vec<HistoryViolation>, git2::Error> {
    let snapshots = collect_history_snapshots(repo_path, baseline)?;
    Ok(check_history_snapshots(&snapshots, baseline.is_some()))
}

/// Read .test-status.json from a specific commit's tree.
fn status_file_at_commit(
    repo: &git2::Repository,
    oid: git2::Oid,
) -> Result<Option<StatusFile>, git2::Error> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let entry = match tree.get_name(".test-status.json") {
        Some(e) => e,
        None => return Ok(None),
    };

    let blob = repo.find_blob(entry.id())?;
    let content = std::str::from_utf8(blob.content())
        .map_err(|e| git2::Error::from_str(&format!("Invalid UTF-8 in .test-status.json: {e}")))?;

    match serde_json::from_str::<StatusFile>(content) {
        Ok(sf) => Ok(Some(sf)),
        Err(e) => Err(git2::Error::from_str(&format!(
            "Failed to parse .test-status.json at {}: {}",
            oid, e
        ))),
    }
}
