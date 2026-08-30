use std::fs;

#[test]
fn privileged_ledger_workflow_enforces_the_writer_boundary() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml")
        .expect("a trusted ledger writer workflow must exist");

    assert!(
        workflow.contains("pull_request_target:"),
        "the writer workflow must come from trusted base-branch code"
    );
    assert!(
        workflow.contains("github.actor != 'github-actions[bot]'"),
        "bot-authored status commits must not start an infinite writer loop"
    );
    assert!(
        workflow.contains("contents: read") && workflow.contains("contents: write"),
        "untrusted test execution and the isolated writer need different token permissions"
    );
    assert!(
        workflow.contains("persist-credentials: false"),
        "the job that runs pull-request code must not retain write credentials"
    );
    assert!(
        workflow.contains("verification.verified")
            && workflow.contains("github-actions[bot]")
            && workflow.contains(".test-status.json"),
        "ordinary pull-request commits that edit the ledger must be rejected"
    );
    assert!(
        workflow.contains("actions/upload-artifact")
            && workflow.contains("actions/download-artifact")
            && workflow.contains("repos/$REPOSITORY/contents/.test-status.json"),
        "the isolated writer must commit exactly the ratchet-produced artifact through the Contents API"
    );
}

#[test]
fn trusted_ratchet_release_is_version_and_digest_pinned() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml").unwrap();

    assert!(
        workflow.contains("releases/download/v1.0.3/cargo-ratchet-x86_64-linux"),
        "validation must not execute a mutable latest-release URL"
    );
    assert!(
        workflow.contains("771d6beb41e425dbe0ce3e024c963fc6f2316ea16d2699073fcb81992610bad0")
            && workflow.contains("sha256sum --check"),
        "the base-controlled workflow must authenticate the pinned ratchet binary"
    );
}

#[test]
fn writer_semantically_revalidates_untrusted_artifact() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml").unwrap();

    assert!(
        workflow.contains("previous-ledger.json")
            && workflow.contains("instructions.json")
            && workflow.contains("semantic ledger transition"),
        "the writer must compare the artifact with trusted HEAD state and declared instructions"
    );
    assert!(
        workflow.contains("new tests must enter pending")
            && workflow.contains("passing tests cannot be downgraded")
            && workflow.contains("removed tests require an instruction"),
        "shape validation alone cannot establish a trusted state transition"
    );
}

#[test]
fn writer_serializes_runs_and_commits_against_verified_head() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml").unwrap();

    assert!(
        workflow.contains("group: ledger-pr-${{ github.event.pull_request.number }}"),
        "runs for one pull request must be serialized"
    );
    assert!(
        workflow.contains("git/commits")
            && workflow.contains("parents: [$head_sha]")
            && workflow.contains("git/refs/heads/$HEAD_REF")
            && workflow.contains("force=false"),
        "the status commit must use the verified PR head as its parent and reject stale ref updates"
    );
}
