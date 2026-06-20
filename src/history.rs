// Git history inspection: verify no test skipped the pending state.

use crate::ratchet::GATEKEEPER_TEST_NAME;
use crate::status::{StatusFile, TestState};
use std::collections::{BTreeMap, BTreeSet};
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
/// appearance as "pending". The gatekeeper test is always exempt.
///
/// `adoption_snapshot_idx` is the index (into the oldest→newest `snapshots`
/// list) of the adoption snapshot: the first status snapshot at or after the
/// point the project began enforcing the ratchet. Tests whose first appearance
/// is at or before that index are trusted (the one sanctioned bootstrap window);
/// every test first appearing as "passing" after it must earn red→green.
///
/// In the bootstrap / pre-adoption case (no resolvable baseline), the caller
/// passes `0`, so the first snapshot is the adoption snapshot — reproducing the
/// old first-snapshot trust behavior exactly.
pub fn check_history_snapshots(
    snapshots: &[HistorySnapshot],
    adoption_snapshot_idx: usize,
) -> Vec<HistoryViolation> {
    let mut first_seen = BTreeMap::new();
    let mut identity_aliases = BTreeMap::new();
    let mut violations = Vec::new();
    let active_identities = active_history_identities(snapshots);

    for (idx, snapshot) in snapshots.iter().enumerate() {
        record_history_renames(&mut identity_aliases, &snapshot.status);

        for (test_name, state) in &snapshot.status.tests {
            let identity_name = resolve_history_identity(&identity_aliases, test_name);

            if !active_identities.contains(identity_name) {
                continue;
            }

            if !mark_first_appearance(&mut first_seen, identity_name) {
                continue;
            }

            if *state != TestState::Passing {
                continue;
            }

            if !is_grandfathered(identity_name, idx, adoption_snapshot_idx) {
                violations.push(HistoryViolation::SkippedPending {
                    test: test_name.clone(),
                    commit: snapshot.commit.clone(),
                });
            }
        }
    }

    violations
}

/// Resolve the adoption-snapshot index from the latest snapshot's `baseline`
/// field. Returns `0` (bootstrap) when there is no baseline, or when the
/// baseline SHA does not resolve to a commit / has no snapshot at-or-before it.
///
/// When the baseline resolves, the adoption snapshot is the first status
/// snapshot strictly after the newest snapshot whose commit is an
/// ancestor-or-equal of the baseline SHA.
pub fn adoption_snapshot_index(
    repo_path: &Path,
    snapshots: &[HistorySnapshot],
) -> Result<usize, git2::Error> {
    let Some(baseline) = snapshots.last().and_then(|s| s.status.baseline.clone()) else {
        return Ok(0);
    };

    let repo = git2::Repository::open(repo_path)?;
    let Ok(baseline_oid) = git2::Oid::from_str(&baseline) else {
        return Ok(0);
    };
    if repo.find_commit(baseline_oid).is_err() {
        return Ok(0);
    }

    // b_idx = newest snapshot whose commit is an ancestor-or-equal of baseline.
    let mut b_idx = None;
    for (idx, snapshot) in snapshots.iter().enumerate() {
        let Ok(oid) = git2::Oid::from_str(&snapshot.commit) else {
            continue;
        };
        let is_ancestor_or_equal =
            oid == baseline_oid || repo.graph_descendant_of(baseline_oid, oid).unwrap_or(false);
        if is_ancestor_or_equal {
            b_idx = Some(idx);
        }
    }

    match b_idx {
        Some(b) => Ok(b + 1),
        None => Ok(0),
    }
}

fn active_history_identities(snapshots: &[HistorySnapshot]) -> BTreeSet<String> {
    let Some(latest_snapshot) = snapshots.last() else {
        return BTreeSet::new();
    };

    let mut final_aliases = BTreeMap::new();
    for snapshot in snapshots {
        record_history_renames(&mut final_aliases, &snapshot.status);
    }

    latest_snapshot
        .status
        .tests
        .keys()
        .map(|test_name| resolve_history_identity(&final_aliases, test_name).to_string())
        .collect()
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
    first_appearance_idx: usize,
    adoption_snapshot_idx: usize,
) -> bool {
    is_gatekeeper(test_name) || first_appearance_idx <= adoption_snapshot_idx
}

fn is_gatekeeper(test_name: &str) -> bool {
    test_name.ends_with(GATEKEEPER_TEST_NAME)
}

/// Convenience: collect snapshots and check them in one call.
/// Resolves the adoption snapshot from the latest snapshot's baseline field.
pub fn check_history(repo_path: &Path) -> Result<Vec<HistoryViolation>, git2::Error> {
    let snapshots = collect_history_snapshots(repo_path)?;
    let adoption_idx = adoption_snapshot_index(repo_path, &snapshots)?;
    Ok(check_history_snapshots(&snapshots, adoption_idx))
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

    match StatusFile::parse_historical_from_str(content, Path::new(".test-status.json")) {
        Ok(sf) => Ok(Some(sf)),
        Err(e) => Err(git2::Error::from_str(&format!(
            "Failed to parse .test-status.json at {}: {}",
            oid, e
        ))),
    }
}
