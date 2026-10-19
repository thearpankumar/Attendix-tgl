//! Authorization and validation gaps: mentors editing schedules/config,
//! super-admin WebAuthn management of off-roster passkeys.

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn client_for(app: &axum::Router, db: &sqlx::PgPool, role: &str) -> (Client, Uuid) {
    let u = unique(&format!(
        "authz-{}",
        if role == "admin" { "mentor" } else { "super" }
    ));
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, role).await;
    (Client::login(app, &u, PW).await, id)
}

/// F4: an exam session's assigned mentor can read it but not rewrite its
/// schedule; moving only `startsAt` slides the attendance window with it.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn assigned_mentor_cannot_edit_schedule_and_start_moves_the_window() {
    let (app, db) = create_test_app().await;
    let (sup, sup_id) = client_for(&app, &db, "super_admin").await;
    let (mentor, mentor_id) = client_for(&app, &db, "admin").await;
    let (location_id, batch_id) = seed_location_and_batch(&db, sup_id, "AZ001").await;

    let (status, body) = sup
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location_id.to_string(),
                "batchId": batch_id.to_string(),
                "assignedAdminIds": [mentor_id],
                "collegeName": "XYZ College",
                "startsAt": chrono::Utc::now().to_rfc3339(),
                "durationMinutes": 60,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let session_id: Uuid = body["_id"].as_str().unwrap().parse().unwrap();
    // Pin the session to 12:00 IST today. The same-day edit guard compares IST calendar
    // days, so pinning to a UTC day flakes for the 5.5 hours after 18:30 UTC.
    sqlx::query(
        "UPDATE sessions SET starts_at = date_trunc('day', now() AT TIME ZONE 'Asia/Kolkata') AT TIME ZONE 'Asia/Kolkata' + interval '12 hours', \
         expires_at = date_trunc('day', now() AT TIME ZONE 'Asia/Kolkata') AT TIME ZONE 'Asia/Kolkata' + interval '13 hours' WHERE id = $1",
    )
    .bind(session_id)
    .execute(&db)
    .await
    .unwrap();

    let uri = format!("/api/admin/sessions/{session_id}/schedule");
    let new_start = (chrono::Utc::now() + chrono::Duration::hours(30)).to_rfc3339();

    // The assigned mentor can still see the session ...
    let (status, _) = mentor
        .get(&app, &format!("/api/admin/sessions/{session_id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    // ... but not change its schedule.
    let (status, body) = mentor
        .mutate(
            &app,
            "PATCH",
            &uri,
            serde_json::json!({ "startsAt": new_start }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body:?}");
    let unchanged: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT expires_at FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        unchanged
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .format("%H:%M")
            .to_string(),
        "13:00"
    );

    // The creator moves only the start: the 60-minute window slides along
    // instead of being stranded at the old expiry.
    let (status, body) = sup
        .mutate(
            &app,
            "PATCH",
            &uri,
            serde_json::json!({ "startsAt": new_start }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let starts: chrono::DateTime<chrono::Utc> = body["startsAt"].as_str().unwrap().parse().unwrap();
    let expires: chrono::DateTime<chrono::Utc> =
        body["expiresAt"].as_str().unwrap().parse().unwrap();
    assert_eq!(expires - starts, chrono::Duration::minutes(60), "{body:?}");
    assert!(expires > chrono::Utc::now());
}

/// F5: system-wide config is super-admin-only to change, validated, audited.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn only_super_admin_can_update_validated_system_config() {
    let (app, db) = create_test_app().await;
    let (sup, sup_id) = client_for(&app, &db, "super_admin").await;
    let (mentor, mentor_id) = client_for(&app, &db, "admin").await;

    // Snapshot the singleton row to restore it (shared test DB).
    let original: Option<(bool, serde_json::Value)> =
        sqlx::query_as("SELECT dev_bypass_enabled, config FROM system_configs LIMIT 1")
            .fetch_optional(&db)
            .await
            .unwrap();

    let (status, cfg) = mentor.get(&app, "/api/config").await;
    assert_eq!(status, StatusCode::OK, "mentors may still read config");

    // Round-trip of the current values, minus the read-only bookkeeping.
    let mut same = cfg.clone();
    for k in ["id", "devBypassEnabled", "updatedBy", "updatedAt"] {
        same.as_object_mut().unwrap().remove(k);
    }

    let (status, body) = mentor
        .mutate(&app, "PUT", "/api/config", same.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    let audit_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_log WHERE event = 'system_config_updated' AND admin_id = $1",
    )
    .bind(mentor_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audit_before, 0, "a refused request is not an update");

    for (label, mutate) in [
        (
            "zero rate-limit window",
            serde_json::json!({ "rateLimits": { "loginWindowSecs": 0 } }),
        ),
        (
            "negative grace period",
            serde_json::json!({ "webauthnConfig": { "gracePeriodMinutes": -5 } }),
        ),
        (
            "similarity above 1",
            serde_json::json!({ "photoVerification": { "similarityThreshold": 1.5 } }),
        ),
        (
            "low threshold above high",
            serde_json::json!({ "photoVerification": { "similarityThreshold": 0.9, "highSimilarityThreshold": 0.5 } }),
        ),
        (
            "zero login attempts",
            serde_json::json!({ "lockoutConfig": { "maxLoginAttempts": 0 } }),
        ),
        (
            "negative early window",
            serde_json::json!({ "sessionConfig": { "manualMarkEarlyWindowMinutes": -1 } }),
        ),
    ] {
        let (status, body) = sup.mutate(&app, "PUT", "/api/config", mutate).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}: {body:?}");
    }

    let (status, body) = sup.mutate(&app, "PUT", "/api/config", same).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    // The dev-bypass flag stays unreachable from this endpoint.
    let (status, _) = sup.mutate(&app, "PUT", "/api/config", cfg.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_log WHERE event = 'system_config_updated' AND admin_id = $1",
    )
    .bind(sup_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(audit, 1);

    match original {
        Some((bypass, config)) => {
            sqlx::query("UPDATE system_configs SET dev_bypass_enabled = $1, config = $2")
                .bind(bypass)
                .bind(config)
                .execute(&db)
                .await
                .unwrap();
        }
        None => {
            sqlx::query("DELETE FROM system_configs")
                .execute(&db)
                .await
                .unwrap();
        }
    }
}

async fn seed_credential(db: &sqlx::PgPool, student_id: &str) {
    sqlx::query(
        "INSERT INTO webauthn_credentials (id, student_id, credential_id, passkey, enrolled_at) \
         VALUES ($1, $2, $3, $4, now())",
    )
    .bind(Uuid::new_v4())
    .bind(student_id)
    .bind(format!("cred-{}", Uuid::new_v4()))
    .bind(vec![1u8, 2, 3])
    .execute(db)
    .await
    .unwrap();
}

/// F8: a super-admin can manage an off-roster passkey they can already see;
/// responses carry a message; acting on a student with no passkey is a 404 and
/// logs nothing; a mentor still can't touch a student outside their rosters.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn super_admin_manages_off_roster_passkeys_and_missing_ones_are_404() {
    let (app, db) = create_test_app().await;
    let (sup, _) = client_for(&app, &db, "super_admin").await;
    let (mentor, _) = client_for(&app, &db, "admin").await;
    let roll = format!("OFF{}", &Uuid::new_v4().simple().to_string()[..8]).to_uppercase();
    seed_credential(&db, &roll).await;

    // Visible to the super-admin (existing behaviour) ...
    let (_, list) = sup
        .get(
            &app,
            &format!("/api/admin/webauthn/credentials?search={roll}"),
        )
        .await;
    assert_eq!(list["pagination"]["total"], 1, "{list:?}");

    // ... and now also manageable.
    for (path, expect) in [
        ("suspend", "suspended"),
        ("unsuspend", "unsuspended"),
        ("reset", "reset"),
    ] {
        let (status, body) = sup
            .mutate(
                &app,
                "POST",
                &format!("/api/admin/webauthn/{path}"),
                serde_json::json!({ "student_id": roll, "reason": "test" }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
        assert!(
            body["message"]
                .as_str()
                .unwrap_or_default()
                .contains(expect),
            "{path} must return a message: {body:?}"
        );
    }
    let is_suspended: bool =
        sqlx::query_scalar("SELECT is_suspended FROM webauthn_credentials WHERE student_id = $1")
            .bind(&roll)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(!is_suspended);

    // Case-insensitive roll match.
    let (status, _) = sup
        .mutate(
            &app,
            "POST",
            "/api/admin/webauthn/suspend",
            serde_json::json!({ "student_id": roll.to_lowercase(), "reason": "test" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // A student with no passkey: 404, and no re-enrollment log entry.
    let ghost = format!("GHOST{}", &Uuid::new_v4().simple().to_string()[..8]).to_uppercase();
    for path in ["suspend", "unsuspend", "reset"] {
        let (status, body) = sup
            .mutate(
                &app,
                "POST",
                &format!("/api/admin/webauthn/{path}"),
                serde_json::json!({ "student_id": ghost, "reason": "test" }),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body:?}");
    }
    let logs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM webauthn_reenrollment_logs WHERE student_id = $1")
            .bind(&ghost)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(logs, 0);

    // A mentor with no such student on their rosters still gets 404.
    let (status, _) = mentor
        .mutate(
            &app,
            "POST",
            "/api/admin/webauthn/suspend",
            serde_json::json!({ "student_id": roll, "reason": "test" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
