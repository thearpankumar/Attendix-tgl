//! Coverage for `models::audit_log` itself (previously untested), plus
//! regression guards for the 2026-08 audit's "flagged attendance never enters
//! the tamper-evident chain" finding — `record_audit_event` used to be called
//! only from admin-side actions.

use attendance_geotag_backend::models::{record_audit_event, verify_chain};

use crate::test_db;

// `admin_audit_log` is deliberately excluded from `cleanup_test_db`'s
// TRUNCATE list (it's append-only/tamper-evident by design, and this
// container is `with_reuse(Always)` — shared across every test binary and
// every run, forever). That means these tests must (a) identify their own
// rows by a unique-per-run marker rather than by position/count, since
// unrelated rows from other test runs persist indefinitely, and (b) never
// leave the chain in a genuinely broken state, since a corrupted row would
// poison `verify_chain` for every future test run against this same
// container. The tamper test below repairs the row it deliberately corrupts
// for exactly this reason.

#[tokio::test]
async fn record_audit_event_appends_a_verifiable_chained_entry() {
    let pool = test_db::get_test_database().await;
    let marker = format!("AUDIT-{}", uuid::Uuid::new_v4());

    record_audit_event(
        &pool,
        None,
        "attendance_flagged",
        serde_json::json!({ "rollNumber": marker, "flagReason": "test" }),
        Some("203.0.113.10"),
    )
    .await
    .unwrap();

    let row: (Option<uuid::Uuid>, String, serde_json::Value) = sqlx::query_as(
        "SELECT admin_id, event, detail FROM admin_audit_log \
         WHERE detail->>'rollNumber' = $1",
    )
    .bind(&marker)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, None, "a student-initiated flag has no admin actor");
    assert_eq!(row.1, "attendance_flagged");
    assert_eq!(row.2["rollNumber"], marker);
}

#[tokio::test]
async fn verify_chain_detects_a_tampered_row_and_self_heals() {
    let pool = test_db::get_test_database().await;
    let marker = format!("AUDIT-{}", uuid::Uuid::new_v4());
    let original_detail = serde_json::json!({ "rollNumber": marker });

    record_audit_event(&pool, None, "attendance_flagged", original_detail.clone(), None)
        .await
        .unwrap();

    let seq: i64 = sqlx::query_scalar("SELECT seq FROM admin_audit_log WHERE detail->>'rollNumber' = $1")
        .bind(&marker)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Simulate direct DB tampering that bypasses the application entirely.
    sqlx::query("UPDATE admin_audit_log SET detail = $1 WHERE seq = $2")
        .bind(serde_json::json!({ "rollNumber": "TAMPERED" }))
        .bind(seq)
        .execute(&pool)
        .await
        .unwrap();

    let broken_at = verify_chain(&pool).await.unwrap();
    assert!(
        broken_at.is_some() && broken_at.unwrap() <= seq,
        "tampering with a row's detail must be detected at or before the tampered row \
         (got {broken_at:?}, tampered seq={seq})"
    );

    // Repair the row so this deliberate corruption doesn't poison every
    // subsequent test run sharing this reused container — restoring the
    // exact original content makes the recomputed hash match the
    // `entry_hash` captured at insert time again.
    sqlx::query("UPDATE admin_audit_log SET detail = $1 WHERE seq = $2")
        .bind(&original_detail)
        .bind(seq)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        verify_chain(&pool).await.unwrap(),
        None,
        "chain must be valid again once the tampered row is restored to its original content"
    );
}

// ===================================================================
// Regression: flagged attendance submissions must reach the audit chain,
// on both the plain and WebAuthn submission paths.
// ===================================================================

#[test]
fn plain_submission_path_audits_flagged_attendance() {
    let source = include_str!("../../src/controllers/attendance.rs");
    let insert_at = source
        .find("let attendance = insert_attendance(&state.db, &attendance).await?;")
        .expect("insert_attendance call must exist");
    let window = &source[insert_at..(insert_at + 1500).min(source.len())];

    assert!(
        window.contains("if should_flag") && window.contains("record_audit_event"),
        "a flagged submission on the plain/legacy path must call record_audit_event \
         shortly after the attendance row is inserted"
    );
}

#[test]
fn webauthn_submission_path_audits_flagged_attendance() {
    let source = include_str!("../../src/controllers/public_webauthn.rs");
    let insert_at = source
        .find(".bind(SqlxJson(integrity_checks))")
        .expect("the attendance INSERT ... RETURNING id call's last bind must exist");
    let window = &source[insert_at..(insert_at + 1000).min(source.len())];

    assert!(
        window.contains("if should_flag") && window.contains("record_audit_event"),
        "a flagged submission on the WebAuthn-verified path must call record_audit_event \
         shortly after the attendance row is inserted"
    );
}

/// Both submission paths must call the shared, unit-tested
/// `vpn_proxy_anomaly_from_ip_info` (see
/// `models::attendance::vpn_proxy_anomaly_tests` for the actual behavioral
/// coverage: proxy-only, hosting-only, both, neither) rather than each
/// re-implementing the proxy/hosting -> anomaly logic inline, which is what
/// let the two copies drift in earlier iterations of this codebase.
#[test]
fn both_submission_paths_call_the_shared_vpn_proxy_anomaly_builder() {
    for rel_path in ["src/controllers/attendance.rs", "src/controllers/public_webauthn.rs"] {
        let source = std::fs::read_to_string(format!(
            "{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            rel_path
        ))
        .unwrap();

        assert!(
            source.contains("vpn_proxy_anomaly_from_ip_info(&ip_info)"),
            "{rel_path} must call the shared vpn_proxy_anomaly_from_ip_info helper"
        );
    }
}
