// Git history inspection: verify no test skipped the pending state.

use crate::ratchet::GATEKEEPER_TEST_NAME;
use crate::status::{StatusFile, TestState};
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
/// Returns snapshots from oldest to newest for every commit that contains a
/// committed .test-status.json. The first snapshot is the implicit baseline.
pub fn collect_history_snapshots(repo_path: &Path) -> Result<Vec<HistorySnapshot>, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;

    let mut snapshots = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

    for oid_result in revwalk {
        let oid = oid_result?;

        if let Some(sf) = status_file_at_commit(&repo, oid)? {
            snapshots.push(HistorySnapshot {
                commit: oid.to_string(),
                status: sf,
            });
        }
    }

    Ok(snapshots)
}

pub fn read_head_status(repo_path: &Path) -> Result<Option<StatusFile>, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;
    let head = repo.head()?.peel_to_commit()?;
    status_file_at_commit(&repo, head.id())
}

/// Check history snapshots for TDD violations. Pure function — no IO.
///
/// Verifies that every test that appears as "passing" had a prior
/// appearance as "pending". Tests in the first committed status snapshot are
/// grandfathered. The gatekeeper test is always exempt.
///
/// Per-test baselines: extracted from the latest committed status snapshot.
/// When a test has a per-test baseline pointing to commit X, history checking
/// for that test starts at X. The test's first appearance at or after X is
/// grandfathered, just like tests in the first committed status snapshot.
pub fn check_history_snapshots(snapshots: &[HistorySnapshot]) -> Vec<HistoryViolation> {
    let mut first_seen = BTreeMap::new();
    let mut identity_aliases = BTreeMap::new();
    let mut violations = Vec::new();

    let first_snapshot_commit = snapshots.first().map(|s| s.commit.clone());

    // Collect per-test baselines from the latest committed status snapshot.
    let per_test_baselines: BTreeMap<String, String> = snapshots
        .last()
        .map(|s| {
            s.status
                .tests
                .iter()
                .filter_map(|(name, entry)| entry.baseline().map(|b| (name.clone(), b.to_string())))
                .collect()
        })
        .unwrap_or_default();

    // Build a commit-to-index map for efficient ordering lookups.
    let commit_index: BTreeMap<&str, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, s)| (s.commit.as_str(), i))
        .collect();

    for snapshot in snapshots {
        record_history_renames(&mut identity_aliases, &snapshot.status);

        for (test_name, entry) in &snapshot.status.tests {
            let identity_name = resolve_history_identity(&identity_aliases, test_name);

            if !mark_first_appearance(&mut first_seen, identity_name) {
                continue;
            }

            let state = entry.state();

            if state != TestState::Passing {
                continue;
            }

            if !is_grandfathered(
                identity_name,
                &snapshot.commit,
                first_snapshot_commit.as_deref(),
                &per_test_baselines,
                &commit_index,
            ) {
                violations.push(HistoryViolation::SkippedPending {
                    test: test_name.clone(),
                    commit: snapshot.commit.clone(),
                });
            }
        }
    }

    violations
}

fn record_history_renames(identity_aliases: &mut BTreeMap<String, String>, status: &StatusFile) {
    for (new_name, old_name) in &status.renames {
        let canonical_old_name = resolve_history_identity(identity_aliases, old_name).to_string();
        identity_aliases.insert(new_name.clone(), canonical_old_name);
    }
}

fn resolve_history_identity<'a>(
    identity_aliases: &'a BTreeMap<String, String>,
    test_name: &'a str,
) -> &'a str {
    let mut current = test_name;
    while let Some(next) = identity_aliases.get(current) {
        current = next;
    }
    current
}

fn mark_first_appearance(first_seen: &mut BTreeMap<String, ()>, test_name: &str) -> bool {
    first_seen.insert(test_name.to_string(), ()).is_none()
}

fn is_grandfathered(
    test_name: &str,
    snapshot_commit: &str,
    first_snapshot_commit: Option<&str>,
    per_test_baselines: &BTreeMap<String, String>,
    commit_index: &BTreeMap<&str, usize>,
) -> bool {
    is_gatekeeper(test_name)
        || first_snapshot_commit.is_some_and(|first| snapshot_commit == first)
        || is_grandfathered_by_per_test_baseline(
            test_name,
            snapshot_commit,
            per_test_baselines,
            commit_index,
        )
}

fn is_gatekeeper(test_name: &str) -> bool {
    test_name.ends_with(GATEKEEPER_TEST_NAME)
}

fn is_grandfathered_by_per_test_baseline(
    test_name: &str,
    snapshot_commit: &str,
    per_test_baselines: &BTreeMap<String, String>,
    commit_index: &BTreeMap<&str, usize>,
) -> bool {
    per_test_baselines
        .get(test_name)
        .is_some_and(|baseline_commit| {
            let snapshot_idx = commit_index.get(snapshot_commit);
            let baseline_idx = commit_index.get(baseline_commit.as_str());
            match (snapshot_idx, baseline_idx) {
                (Some(&snapshot_idx), Some(&baseline_idx)) => snapshot_idx >= baseline_idx,
                (Some(_), None) => true,
                _ => false,
            }
        })
}

/// Convenience: collect snapshots and check them in one call.
/// Used by existing callers that don't need the split.
pub fn check_history(repo_path: &Path) -> Result<Vec<HistoryViolation>, git2::Error> {
    let snapshots = collect_history_snapshots(repo_path)?;
    Ok(check_history_snapshots(&snapshots))
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
