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
fn validator_recognizes_the_signed_git_data_api_writer_identity() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml").unwrap();

    assert!(
        workflow.contains(".author.login')\" = 'github-actions[bot]'")
            && workflow.contains(".committer.login')\" = 'web-flow'"),
        "GitHub records Git Data API commits with the Actions bot as author and web-flow as signed committer"
    );
}

#[test]
fn validator_builds_the_ratchet_from_base_controlled_source() {
    let workflow = fs::read_to_string(".github/workflows/ledger.yml").unwrap();

    assert!(
        workflow.contains("ref: ${{ github.event.pull_request.base.sha }}")
            && workflow.contains("path: trusted-ratchet-source")
            && workflow.contains("--manifest-path trusted-ratchet-source/Cargo.toml"),
        "the enforcing binary must be built from reviewed base-branch source"
    );
    assert!(
        workflow.contains("path: pull-request")
            && workflow.contains("../trusted-ratchet-source/target/release/cargo-ratchet"),
        "the base-controlled ratchet must execute against a separate pull-request checkout"
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
