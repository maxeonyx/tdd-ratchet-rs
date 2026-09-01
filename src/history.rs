// Git history inspection: verify no test skipped the pending state.

use crate::ratchet::GATEKEEPER_TEST_NAME;
use crate::status::{StatusFile, TestState};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub enum HistoryViolation {
    /// A test appeared as passing without ever being pending.
    SkippedPending { test: String, commit: String },
    /// A committed ledger rewrote an earned passing state back to pending.
    StateRewritten {
        test: String,
        commit: String,
        from: TestState,
        to: TestState,
    },
    /// A commit descended from the adoption snapshot without the ledger.
    LedgerDeleted { commit: String },
}

/// A snapshot of the status file at a specific commit.
#[derive(Debug, Clone)]
pub struct HistorySnapshot {
    pub commit: String,
    pub parents: Vec<String>,
    pub status: StatusFile,
}

/// Collect status file snapshots from git history.
///
/// Returns snapshots from oldest to newest for every commit that contains a
/// committed .test-status.json. The first snapshot is the sole adoption
/// snapshot; every later snapshot is validated against the complete record.
pub fn collect_history_snapshots(repo_path: &Path) -> Result<Vec<HistorySnapshot>, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;

    let mut snapshots = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

    for oid_result in revwalk {
        let oid = oid_result?;

        if let Some(sf) = status_file_at_commit(&repo, oid)? {
            let commit = repo.find_commit(oid)?;
            snapshots.push(HistorySnapshot {
                commit: oid.to_string(),
                parents: commit
                    .parent_ids()
                    .map(|parent| parent.to_string())
                    .collect(),
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
/// The first committed snapshot is trusted adoption. After it, every test that
/// first appears passing is a violation, passing can never be rewritten to
/// pending, and violations remain visible even if a later snapshot removes the
/// offending test. The gatekeeper is exempt from the first-appearance rule.
pub fn check_history_snapshots(snapshots: &[HistorySnapshot]) -> Vec<HistoryViolation> {
    let Some(adoption) = snapshots.first() else {
        return Vec::new();
    };
    let snapshots_by_commit: BTreeMap<&str, &HistorySnapshot> = snapshots
        .iter()
        .map(|snapshot| (snapshot.commit.as_str(), snapshot))
        .collect();
    let mut violations = Vec::new();

    for snapshot in snapshots {
        for (test_name, state) in &snapshot.status.tests {
            let ancestor_states =
                collect_ancestor_states(snapshot, test_name, &snapshots_by_commit);

            match state {
                TestState::Passing
                    if snapshot.commit != adoption.commit
                        && !ancestor_states.contains(&TestState::Pending)
                        && !ancestor_states.contains(&TestState::Passing)
                        && !is_gatekeeper(test_name) =>
                {
                    violations.push(HistoryViolation::SkippedPending {
                        test: test_name.clone(),
                        commit: snapshot.commit.clone(),
                    });
                }
                TestState::Pending if ancestor_states.contains(&TestState::Passing) => {
                    violations.push(HistoryViolation::StateRewritten {
                        test: test_name.clone(),
                        commit: snapshot.commit.clone(),
                        from: TestState::Passing,
                        to: TestState::Pending,
                    });
                }
                _ => {}
            }
        }
    }

    violations
}

fn collect_ancestor_states(
    snapshot: &HistorySnapshot,
    test_name: &str,
    snapshots_by_commit: &BTreeMap<&str, &HistorySnapshot>,
) -> Vec<TestState> {
    let mut initial_names = BTreeSet::from([test_name.to_string()]);
    expand_identity_names(&mut initial_names, &snapshot.status);

    let mut pending = snapshot
        .parents
        .iter()
        .map(|parent| (parent.clone(), initial_names.clone()))
        .collect::<Vec<_>>();
    let mut seen_names_by_commit: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut states = Vec::new();

    while let Some((commit, mut names)) = pending.pop() {
        let Some(ancestor) = snapshots_by_commit.get(commit.as_str()) else {
            continue;
        };
        expand_identity_names(&mut names, &ancestor.status);

        let seen_names = seen_names_by_commit.entry(commit).or_default();
        let new_names = names.difference(seen_names).cloned().collect::<Vec<_>>();
        if new_names.is_empty() {
            continue;
        }
        seen_names.extend(new_names);

        for name in &names {
            if let Some(state) = ancestor.status.tests.get(name) {
                states.push(*state);
            }
        }
        pending.extend(
            ancestor
                .parents
                .iter()
                .map(|parent| (parent.clone(), names.clone())),
        );
    }

    states
}

fn expand_identity_names(names: &mut BTreeSet<String>, status: &StatusFile) {
    loop {
        let old_names = status
            .renames
            .iter()
            .filter(|(new_name, _)| names.contains(*new_name))
            .map(|(_, old_name)| old_name.clone())
            .collect::<Vec<_>>();
        let previous_len = names.len();
        names.extend(old_names);
        if names.len() == previous_len {
            break;
        }
    }
}

fn is_gatekeeper(test_name: &str) -> bool {
    test_name.ends_with(GATEKEEPER_TEST_NAME)
}

/// Convenience: collect snapshots and check them in one call.
pub fn check_history(repo_path: &Path) -> Result<Vec<HistoryViolation>, git2::Error> {
    let snapshots = collect_history_snapshots(repo_path)?;
    let mut violations = check_history_snapshots(&snapshots);
    violations.extend(check_ledger_continuity(repo_path, &snapshots)?);
    Ok(violations)
}

fn check_ledger_continuity(
    repo_path: &Path,
    snapshots: &[HistorySnapshot],
) -> Result<Vec<HistoryViolation>, git2::Error> {
    let Some(adoption) = snapshots.first() else {
        return Ok(Vec::new());
    };

    let repo = git2::Repository::open(repo_path)?;
    let adoption_oid = git2::Oid::from_str(&adoption.commit)?;
    let mut violations = Vec::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    for oid_result in revwalk {
        let oid = oid_result?;
        let is_adoption_descendant =
            oid == adoption_oid || repo.graph_descendant_of(oid, adoption_oid)?;
        if is_adoption_descendant && status_file_at_commit(&repo, oid)?.is_none() {
            violations.push(HistoryViolation::LedgerDeleted {
                commit: oid.to_string(),
            });
        }
    }

    Ok(violations)
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
