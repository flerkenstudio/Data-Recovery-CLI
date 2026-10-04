use recovery_core::RecoverySession;
use test_utils::fixture_path;

#[test]
fn test_synthetic_ntfs_recovery_slice() {
    let img_path = fixture_path("synthetic_ntfs.img");
    assert!(img_path.exists(), "synthetic_ntfs.img fixture missing");

    let mut session =
        RecoverySession::open(&img_path).expect("Failed to open synthetic NTFS image");
    let candidates = session
        .scan(Some(16), |_| {})
        .expect("Failed to scan synthetic NTFS image");

    assert_eq!(candidates.len(), 2, "Expected 2 deleted candidate files");

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let candidate_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();

    let report = session
        .recover_candidates(&candidate_ids, temp_dir.path(), |_| {})
        .expect("Failed to recover candidates");

    assert_eq!(report.total_requested, 2);
    assert_eq!(report.total_successful, 2);
    assert_eq!(report.total_failed, 0);

    let notes_outcome = report
        .outcomes
        .iter()
        .find(|o| o.file_name == "notes.txt")
        .unwrap();
    assert_eq!(
        notes_outcome.sha256_hash.as_deref(),
        Some("5cc4c63a20381f976b4a8f4a2ccde008dfa1245feedb1f3365c2886a2a48e29f")
    );

    let payload_outcome = report
        .outcomes
        .iter()
        .find(|o| o.file_name == "payload.bin")
        .unwrap();
    assert_eq!(
        payload_outcome.sha256_hash.as_deref(),
        Some("7a846dbc846d1dcc0c90432ae0d71b71804a69451d6882724b1e78189782b50a")
    );
}
