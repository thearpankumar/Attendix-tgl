//! Regression coverage for `POST /api/admin/recurring-rules` supporting
//! every `shortlinkMode` the one-off session-creation endpoint supports
//! ("none", "auto", "custom", "existing") — not just "custom"/"existing".
//! `create_recurring_rule`'s match previously had no "auto" arm, so the
//! admin frontend's default cadence (`shortlinkMode: 'auto'`) always failed
//! with "Unknown shortlink mode 'auto'".

use axum::http::StatusCode;
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, Client};

async fn seed_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind("Recurring Rule Test Location")
    .bind(12.9716_f64)
    .bind(77.5946_f64)
    .bind(100.0_f64)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn base_payload(location_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "locationId": location_id.to_string(),
        "description": "Regression test rule",
        "durationMinutes": 30,
        "runTimesLocal": ["09:00"],
        "timezone": "UTC",
        "daysOfWeek": [0, 1, 2, 3, 4, 5, 6],
    })
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn create_recurring_rule_supports_every_shortlink_mode() {
    let (app, db) = create_test_app().await;
    let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
    let email = format!("{username}@example.com");
    seed_admin(
        &db,
        &username,
        &email,
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
        .bind(&username)
        .fetch_one(&db)
        .await
        .unwrap();
    let location_id = seed_location(&db, admin_id).await;

    // "none" — no short link at all.
    let mut payload = base_payload(location_id);
    payload["shortlinkMode"] = serde_json::json!("none");
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "shortlinkMode 'none' should succeed: {body:?}"
    );
    assert!(body["lockedShortCode"].is_null());

    // "auto" — this is the exact regression: the frontend's default mode.
    let mut payload = base_payload(location_id);
    payload["shortlinkMode"] = serde_json::json!("auto");
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "shortlinkMode 'auto' should succeed: {body:?}"
    );
    let auto_code = body["lockedShortCode"]
        .as_str()
        .expect("auto mode must lock a generated short code")
        .to_string();
    assert!(!auto_code.is_empty());

    // "custom" — a fresh, caller-chosen code.
    let custom_code = format!("rrule-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let mut payload = base_payload(location_id);
    payload["shortlinkMode"] = serde_json::json!("custom");
    payload["customShortCode"] = serde_json::json!(custom_code);
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "shortlinkMode 'custom' should succeed: {body:?}"
    );
    assert_eq!(body["lockedShortCode"], serde_json::json!(custom_code));

    // "existing" — reuse the code "auto" mode just generated, now unattached
    // to any active rule (that rule's own occurrences haven't fired yet, but
    // the code itself is a plain unattached short_links row until they do —
    // pick a fresh one instead to avoid the active-rule-lock guard).
    let existing_code = format!(
        "rrule-existing-{}",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    sqlx::query(
        "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, click_count, created_at) \
         VALUES ($1, $2, NULL, $3, true, 0, now())",
    )
    .bind(Uuid::new_v4())
    .bind(&existing_code)
    .bind(admin_id)
    .execute(&db)
    .await
    .unwrap();
    let mut payload = base_payload(location_id);
    payload["shortlinkMode"] = serde_json::json!("existing");
    payload["existingShortCode"] = serde_json::json!(existing_code);
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "shortlinkMode 'existing' should succeed: {body:?}"
    );
    assert_eq!(body["lockedShortCode"], serde_json::json!(existing_code));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn create_recurring_rule_rejects_a_genuinely_unknown_shortlink_mode() {
    let (app, db) = create_test_app().await;
    let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
    let email = format!("{username}@example.com");
    seed_admin(
        &db,
        &username,
        &email,
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;

    let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
        .bind(&username)
        .fetch_one(&db)
        .await
        .unwrap();
    let location_id = seed_location(&db, admin_id).await;

    let mut payload = base_payload(location_id);
    payload["shortlinkMode"] = serde_json::json!("not-a-real-mode");
    let (status, body) = client
        .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a genuinely unknown mode should still 400: {body:?}"
    );
}

/// Regression coverage for reassigning a short code that's already locked to
/// an active recurring rule. Previously this was a hard, unrecoverable 400
/// everywhere — `get_short_links` hid such codes from the admin picker
/// entirely, and both creation endpoints rejected reusing one outright. Now
/// the picker surfaces them (annotated with the owning rule) and an explicit
/// `forceReassign: true` lets an admin confirm taking the code back, which
/// clears the old rule's lock (same effect as pausing it) and hands the code
/// to the new session/rule.
mod reassignment {
    use super::*;

    async fn create_locked_rule(
        app: &axum::Router,
        client: &Client,
        location_id: Uuid,
        code: &str,
    ) -> Uuid {
        let mut payload = base_payload(location_id);
        payload["shortlinkMode"] = serde_json::json!("custom");
        payload["customShortCode"] = serde_json::json!(code);
        let (status, body) = client
            .mutate(app, "POST", "/api/admin/recurring-rules", payload)
            .await;
        assert_eq!(status, StatusCode::CREATED, "response body: {body:?}");
        assert_eq!(body["lockedShortCode"], serde_json::json!(code));
        Uuid::parse_str(body["_id"].as_str().expect("rule id")).unwrap()
    }

    #[tokio::test]
    #[file_serial(admin_bootstrap)]
    async fn get_short_links_annotates_a_rule_locked_code_instead_of_hiding_it() {
        let (app, db) = create_test_app().await;
        let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
        let email = format!("{username}@example.com");
        seed_admin(&db, &username, &email, "correct-horse-battery-staple", "super_admin").await;
        let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
        let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
        let location_id = seed_location(&db, admin_id).await;

        let code = format!("locked-{}", &Uuid::new_v4().simple().to_string()[..8]);
        create_locked_rule(&app, &client, location_id, &code).await;

        let (status, body) = client.get(&app, "/api/admin/shortlinks?limit=200").await;
        assert_eq!(status, StatusCode::OK, "response body: {body:?}");
        let links = body["shortLinks"].as_array().expect("shortLinks array");
        let entry = links
            .iter()
            .find(|l| l["shortCode"] == serde_json::json!(code))
            .unwrap_or_else(|| panic!("locked code missing from list entirely: {links:?}"));
        assert!(
            entry["lockedByRuleId"].is_string(),
            "a rule-locked code must be annotated, not omitted: {entry:?}"
        );
    }

    #[tokio::test]
    #[file_serial(admin_bootstrap)]
    async fn one_off_session_cannot_steal_a_rule_locked_code_without_force_reassign() {
        let (app, db) = create_test_app().await;
        let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
        let email = format!("{username}@example.com");
        seed_admin(&db, &username, &email, "correct-horse-battery-staple", "super_admin").await;
        let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
        let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
        let location_id = seed_location(&db, admin_id).await;

        let code = format!("locked-{}", &Uuid::new_v4().simple().to_string()[..8]);
        create_locked_rule(&app, &client, location_id, &code).await;

        let (status, body) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/sessions",
                serde_json::json!({
                    "locationId": location_id.to_string(),
                    "durationMinutes": 30,
                    "shortlinkMode": "existing",
                    "existingShortCode": code,
                }),
            )
            .await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "without force_reassign, stealing a rule-locked code must still fail: {body:?}"
        );
    }

    #[tokio::test]
    #[file_serial(admin_bootstrap)]
    async fn one_off_session_reassigns_a_rule_locked_code_with_force_reassign() {
        let (app, db) = create_test_app().await;
        let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
        let email = format!("{username}@example.com");
        seed_admin(&db, &username, &email, "correct-horse-battery-staple", "super_admin").await;
        let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
        let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
        let location_id = seed_location(&db, admin_id).await;

        let code = format!("locked-{}", &Uuid::new_v4().simple().to_string()[..8]);
        let rule_id = create_locked_rule(&app, &client, location_id, &code).await;

        let (status, body) = client
            .mutate(
                &app,
                "POST",
                "/api/admin/sessions",
                serde_json::json!({
                    "locationId": location_id.to_string(),
                    "durationMinutes": 30,
                    "shortlinkMode": "existing",
                    "existingShortCode": code,
                    "forceReassign": true,
                }),
            )
            .await;

        assert_eq!(
            status,
            StatusCode::CREATED,
            "with force_reassign, stealing a rule-locked code must succeed: {body:?}"
        );
        assert_eq!(body["shortCode"], serde_json::json!(code));

        let locked_short_code: Option<String> = sqlx::query_scalar(
            "SELECT locked_short_code FROM recurring_session_rules WHERE id = $1",
        )
        .bind(rule_id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(
            locked_short_code.is_none(),
            "the old rule must lose its lock once the code is reassigned away from it"
        );
    }

    #[tokio::test]
    #[file_serial(admin_bootstrap)]
    async fn recurring_rule_cannot_steal_another_rules_locked_code_without_force_reassign() {
        let (app, db) = create_test_app().await;
        let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
        let email = format!("{username}@example.com");
        seed_admin(&db, &username, &email, "correct-horse-battery-staple", "super_admin").await;
        let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
        let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
        let location_id = seed_location(&db, admin_id).await;

        let code = format!("locked-{}", &Uuid::new_v4().simple().to_string()[..8]);
        create_locked_rule(&app, &client, location_id, &code).await;

        let mut payload = base_payload(location_id);
        payload["shortlinkMode"] = serde_json::json!("existing");
        payload["existingShortCode"] = serde_json::json!(code);
        let (status, body) = client
            .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
            .await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "without force_reassign, a second rule stealing another rule's locked code must still fail: {body:?}"
        );
    }

    #[tokio::test]
    #[file_serial(admin_bootstrap)]
    async fn recurring_rule_reassigns_another_rules_locked_code_with_force_reassign() {
        let (app, db) = create_test_app().await;
        let username = format!("rrule-admin-{}", Uuid::new_v4().simple());
        let email = format!("{username}@example.com");
        seed_admin(&db, &username, &email, "correct-horse-battery-staple", "super_admin").await;
        let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
        let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM admins WHERE username = $1")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
        let location_id = seed_location(&db, admin_id).await;

        let code = format!("locked-{}", &Uuid::new_v4().simple().to_string()[..8]);
        let old_rule_id = create_locked_rule(&app, &client, location_id, &code).await;

        let mut payload = base_payload(location_id);
        payload["shortlinkMode"] = serde_json::json!("existing");
        payload["existingShortCode"] = serde_json::json!(code);
        payload["forceReassign"] = serde_json::json!(true);
        let (status, body) = client
            .mutate(&app, "POST", "/api/admin/recurring-rules", payload)
            .await;

        assert_eq!(
            status,
            StatusCode::CREATED,
            "with force_reassign, a second rule stealing another rule's locked code must succeed: {body:?}"
        );
        assert_eq!(body["lockedShortCode"], serde_json::json!(code));

        let old_locked_short_code: Option<String> = sqlx::query_scalar(
            "SELECT locked_short_code FROM recurring_session_rules WHERE id = $1",
        )
        .bind(old_rule_id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(
            old_locked_short_code.is_none(),
            "the old rule must lose its lock once the code is reassigned to the new rule"
        );
    }
}
