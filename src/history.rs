// Git history inspection: verify no test skipped the pending state.

use crate::status::StatusFile;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum HistoryViolation {
    /// A test appeared as passing without ever being pending.
    SkippedPending { test: String, commit: String },
}

/// Walk git history and verify all tests went through the pending state.
///
/// `baseline`: if Some, only check commits after (not including) this commit.
/// Tests that existed at or before the baseline are grandfathered.
pub fn check_history(
    repo_path: &Path,
    baseline: Option<&str>,
) -> Result<Vec<HistoryViolation>, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;

    // Collect status file snapshots from each commit, oldest first.
    let mut snapshots: Vec<(String, StatusFile)> = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

    let baseline_oid = baseline.map(|b| git2::Oid::from_str(b)).transpose()?;

    let mut past_baseline = baseline_oid.is_none();

    for oid_result in revwalk {
        let oid = oid_result?;

        if !past_baseline {
            if Some(oid) == baseline_oid {
                // Record the baseline commit's status as the grandfathered set
                if let Some(sf) = status_file_at_commit(&repo, oid)? {
                    let hash = oid.to_string();
                    snapshots.push((hash, sf));
                }
                past_baseline = true;
            }
            continue;
        }

        let hash = oid.to_string();
        if let Some(sf) = status_file_at_commit(&repo, oid)? {
            snapshots.push((hash, sf));
        }
    }

    // Now check: for each test that appears as "passing" in any snapshot,
    // verify it appeared as "pending" in an earlier snapshot.
    let mut first_seen: BTreeMap<String, (String, crate::status::TestState)> = BTreeMap::new();
    let mut violations = Vec::new();

    for (commit_hash, sf) in &snapshots {
        for (test_name, state) in &sf.tests {
            if !first_seen.contains_key(test_name) {
                first_seen.insert(test_name.clone(), (commit_hash.clone(), *state));
                // If a test's first appearance is "passing", that's a violation
                // (unless it was in the baseline commit — those are grandfathered)
                if *state == crate::status::TestState::Passing {
                    let is_baseline_commit =
                        baseline_oid.is_some_and(|b| commit_hash == &b.to_string());
                    if !is_baseline_commit {
                        violations.push(HistoryViolation::SkippedPending {
                            test: test_name.clone(),
                            commit: commit_hash.clone(),
                        });
                    }
                }
            }
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

    match serde_json::from_str::<StatusFile>(content) {
        Ok(sf) => Ok(Some(sf)),
        Err(e) => Err(git2::Error::from_str(&format!(
            "Failed to parse .test-status.json at {}: {}",
            oid, e
        ))),
    }
}
