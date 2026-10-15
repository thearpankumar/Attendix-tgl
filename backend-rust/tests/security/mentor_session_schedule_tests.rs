//! Mentor (exam) sessions: optional location on direct creation, and the
//! one-time / cron rule path carrying mentors, college and location onto the
//! generated sessions.

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, Client,
};

struct Ctx {
    app: axum::Router,
    db: sqlx::PgPool,
    client: Client,
    admin_id: Uuid,
    mentor_id: Uuid,
    location_id: Uuid,
    batch_id: Uuid,
}

async fn setup() -> Ctx {
    let (app, db) = create_test_app().await;
    let username = format!("mentor-sched-{}", Uuid::new_v4().simple());
    let admin_id = seed_admin(
        &db,
        &username,
        &format!("{username}@example.com"),
        "correct-horse-battery-staple",
        "super_admin",
    )
    .await;
    let client = Client::login(&app, &username, "correct-horse-battery-staple").await;
    let mentor_username = format!("mentor-sched-m-{}", Uuid::new_v4().simple());
    let mentor_id = seed_admin(
        &db,
        &mentor_username,
        &format!("{mentor_username}@example.com"),
        "mentor-password-123",
        "admin",
    )
    .await;
    let (location_id, batch_id) = seed_location_and_batch(
        &db,
        admin_id,
        &format!("MS{}", &Uuid::new_v4().simple().to_string()[..6]),
    )
    .await;
    Ctx {
        app,
        db,
        client,
        admin_id,
        mentor_id,
        location_id,
        batch_id,
    }
}

fn exam_rule(c: &Ctx) -> serde_json::Value {
    serde_json::json!({
        "sessionType": "exam",
        "locationId": c.location_id.to_string(),
        "batchId": c.batch_id.to_string(),
        "assignedAdminIds": [c.mentor_id.to_string()],
        "collegeName": "Rule College",
        "description": "mentor rule",
        "durationMinutes": 60,
        "runTimesLocal": ["09:00"],
        "timezone": "UTC",
        "shortlinkMode": "none",
    })
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn exam_session_with_location_stores_and_returns_it_and_without_still_works() {
    let c = setup().await;
    let base = |loc: Option<Uuid>| {
        let mut v = serde_json::json!({
            "sessionType": "exam",
            "batchId": c.batch_id.to_string(),
            "assignedAdminIds": [c.mentor_id.to_string()],
            "collegeName": "XYZ College",
            "startsAt": (Utc::now() + Duration::hours(1)).to_rfc3339(),
            "durationMinutes": 60,
            "shortlinkMode": "none",
        });
        if let Some(l) = loc {
            v["locationId"] = serde_json::json!(l.to_string());
        }
        v
    };

    let (status, body) = c
        .client
        .mutate(
            &c.app,
            "POST",
            "/api/admin/sessions",
            base(Some(c.location_id)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["locationId"], c.location_id.to_string());
    assert!(body["locationName"].is_string());

    let (status, body) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/sessions", base(None))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert!(body["locationId"].is_null());

    let (status, _) = c
        .client
        .mutate(
            &c.app,
            "POST",
            "/api/admin/sessions",
            base(Some(Uuid::new_v4())),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unknown location is rejected"
    );

    let mut bad = base(None);
    bad["locationId"] = serde_json::json!("not-a-uuid");
    let (status, _) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/sessions", bad)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn exam_rule_validation_and_response_fields() {
    let c = setup().await;

    let (status, body) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/recurring-rules", {
            let mut p = exam_rule(&c);
            p["daysOfWeek"] = serde_json::json!([0, 1, 2, 3, 4, 5, 6]);
            p
        })
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["sessionType"], "exam");
    assert_eq!(body["collegeName"], "Rule College");
    assert_eq!(
        body["assignedAdminIds"],
        serde_json::json!([c.mentor_id.to_string()])
    );

    // An exam rule's location is optional.
    let mut p = exam_rule(&c);
    p["daysOfWeek"] = serde_json::json!([1]);
    p.as_object_mut().unwrap().remove("locationId");
    let (status, body) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert!(body["locationId"].is_null());

    // Missing mentors / college / batch are rejected for an exam rule.
    for field in ["assignedAdminIds", "collegeName", "batchId"] {
        let mut p = exam_rule(&c);
        p["daysOfWeek"] = serde_json::json!([1]);
        p.as_object_mut().unwrap().remove(field);
        let (status, _) = c
            .client
            .mutate(&c.app, "POST", "/api/admin/recurring-rules", p)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "missing {field}");
    }

    // A normal rule still needs a location.
    let mut p = exam_rule(&c);
    p["sessionType"] = serde_json::json!("normal");
    p["daysOfWeek"] = serde_json::json!([1]);
    p.as_object_mut().unwrap().remove("locationId");
    let (status, _) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/recurring-rules", p)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn scheduler_generates_exam_session_with_mentors_college_and_location() {
    let c = setup().await;
    let (status, body) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/recurring-rules", {
            let mut p = exam_rule(&c);
            p["daysOfWeek"] = serde_json::json!([0, 1, 2, 3, 4, 5, 6]);
            p
        })
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let rule_id: Uuid = body["_id"].as_str().unwrap().parse().unwrap();
    sqlx::query("UPDATE recurring_session_rules SET next_run_at = now() - interval '5 seconds' WHERE id = $1")
        .bind(rule_id)
        .execute(&c.db)
        .await
        .unwrap();

    while attendance_geotag_backend::services::claim_and_generate_one(&c.db)
        .await
        .unwrap()
    {}

    let (session_id, location, college, starts): (
        Uuid,
        Option<Uuid>,
        Option<String>,
        Option<chrono::DateTime<Utc>>,
    ) = sqlx::query_as(
        "SELECT id, location_id, college_name, starts_at FROM sessions WHERE recurring_rule_id = $1",
    )
    .bind(rule_id)
    .fetch_one(&c.db)
    .await
    .unwrap();
    assert_eq!(location, Some(c.location_id));
    assert_eq!(college.as_deref(), Some("Rule College"));
    assert!(starts.is_some(), "exam occurrence starts at fire time");
    let mentors: Vec<Uuid> =
        sqlx::query_scalar("SELECT admin_id FROM session_admins WHERE session_id = $1")
            .bind(session_id)
            .fetch_all(&c.db)
            .await
            .unwrap();
    assert_eq!(mentors, vec![c.mentor_id]);
    let _ = c.admin_id;
}

#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn one_time_normal_rule_with_mentors_assigns_them_to_the_generated_session() {
    let c = setup().await;
    let date = (Utc::now() + Duration::days(2)).date_naive();
    let (status, body) = c
        .client
        .mutate(&c.app, "POST", "/api/admin/recurring-rules", {
            serde_json::json!({
                "locationId": c.location_id.to_string(),
                "assignedAdminIds": [c.mentor_id.to_string()],
                "durationMinutes": 30,
                "runTimesLocal": ["09:00"],
                "timezone": "UTC",
                "runOnceDate": date.to_string(),
                "shortlinkMode": "none",
            })
        })
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let rule_id: Uuid = body["_id"].as_str().unwrap().parse().unwrap();
    sqlx::query("UPDATE recurring_session_rules SET next_run_at = now() - interval '5 seconds' WHERE id = $1")
        .bind(rule_id)
        .execute(&c.db)
        .await
        .unwrap();
    while attendance_geotag_backend::services::claim_and_generate_one(&c.db)
        .await
        .unwrap()
    {}
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_admins sa JOIN sessions s ON s.id = sa.session_id \
         WHERE s.recurring_rule_id = $1 AND sa.admin_id = $2",
    )
    .bind(rule_id)
    .bind(c.mentor_id)
    .fetch_one(&c.db)
    .await
    .unwrap();
    assert_eq!(n, 1);
}
