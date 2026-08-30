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
