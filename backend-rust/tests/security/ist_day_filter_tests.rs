//! The product is IST-only: a "day" filter/bucket is the half-open interval
//! [00:00 IST, 00:00 IST next day), matched on `COALESCE(starts_at,
//! created_at)` for sessions. These tests plant rows on both sides of IST
//! midnight (18:30:00Z) and check which day returns them.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serial_test::file_serial;
use uuid::Uuid;

use crate::exam_session_flow_tests::{create_test_app, seed_admin, unique, Client};

const PW: &str = "correct-horse-battery-staple";

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

async fn insert_session(
    db: &sqlx::PgPool,
    admin: Uuid,
    desc: &str,
    created_at: &str,
    starts_at: Option<&str>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, token_prefix, description, created_by, starts_at, \
         is_active, expires_at, created_at) VALUES ($1, $2, 'ist', $3, $4, $5, true, $6, $7)",
    )
    .bind(id)
    .bind(format!("hash-{id}"))
    .bind(desc)
    .bind(admin)
    .bind(starts_at.map(ts))
    .bind(ts(created_at) + chrono::Duration::hours(10))
    .bind(ts(created_at))
    .execute(db)
    .await
    .unwrap();
    id
}

async fn insert_attendance(db: &sqlx::PgPool, session_id: Uuid, roll: &str, at: &str) {
    sqlx::query(
        "INSERT INTO attendances (id, session_id, student_name, roll_number, photo_url, \
         photo_public_id, student_latitude, student_longitude, distance_from_location, verified, \
         source, status, captured_at) \
         VALUES ($1, $2, 'S', $3, 'u', '', 0, 0, 0, true, 'self_submitted', 'present', $4)",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll)
    .bind(ts(at))
    .execute(db)
    .await
    .unwrap();
}

fn ids(body: &serde_json::Value, key: &str) -> Vec<String> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|v| v[key].as_str().unwrap().to_string())
        .collect()
}

/// Far-future dates so rows other tests create with `now()` never collide.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn sessions_list_day_filter_uses_ist_half_open_day_and_coalesces_starts_at() {
    let (app, db) = create_test_app().await;
    let u = unique("ist-super");
    let admin = seed_admin(&db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    let client = Client::login(&app, &u, PW).await;

    // 2031-03-09 23:59:59 IST, 2031-03-10 00:00:00 IST, and the last second of the 10th.
    let before = insert_session(&db, admin, "ist-before", "2031-03-09T18:29:59Z", None).await;
    let at = insert_session(&db, admin, "ist-at", "2031-03-09T18:30:00Z", None).await;
    let last = insert_session(&db, admin, "ist-last", "2031-03-10T18:29:59Z", None).await;
    let next = insert_session(&db, admin, "ist-next", "2031-03-10T18:30:00Z", None).await;
    // Scheduled exam created days earlier but starting on the 10th (IST).
    let exam = insert_session(
        &db,
        admin,
        "ist-exam",
        "2031-03-07T10:00:00Z",
        Some("2031-03-10T05:00:00Z"),
    )
    .await;

    let (status, day9) = client
        .get(&app, "/api/admin/sessions?date=2031-03-09")
        .await;
    assert_eq!(status, StatusCode::OK);
    let day9 = ids(&day9, "_id");
    assert!(day9.contains(&before.to_string()), "{day9:?}");
    for other in [at, last, next, exam] {
        assert!(
            !day9.contains(&other.to_string()),
            "{other} leaked into the 9th"
        );
    }

    let (_, day10) = client
        .get(&app, "/api/admin/sessions?date=2031-03-10")
        .await;
    let day10 = ids(&day10, "_id");
    for want in [at, last, exam] {
        assert!(
            day10.contains(&want.to_string()),
            "{want} missing from the 10th: {day10:?}"
        );
    }
    for other in [before, next] {
        assert!(
            !day10.contains(&other.to_string()),
            "{other} leaked into the 10th"
        );
    }

    let (_, day11) = client
        .get(&app, "/api/admin/sessions?date=2031-03-11")
        .await;
    assert!(ids(&day11, "_id").contains(&next.to_string()));

    // The scheduled exam is not found under the day it was created.
    let (_, day7) = client
        .get(&app, "/api/admin/sessions?date=2031-03-07")
        .await;
    assert!(!ids(&day7, "_id").contains(&exam.to_string()));
}

#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn dashboard_drill_down_buckets_checkins_by_ist_day() {
    let (app, db) = create_test_app().await;
    let u = unique("ist-dash");
    let admin = seed_admin(&db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    let client = Client::login(&app, &u, PW).await;

    let s_before = insert_session(&db, admin, "ist-d-before", "2032-05-01T00:00:00Z", None).await;
    let s_at = insert_session(&db, admin, "ist-d-at", "2032-05-01T00:00:00Z", None).await;
    let s_exam = insert_session(
        &db,
        admin,
        "ist-d-exam",
        "2032-04-28T00:00:00Z",
        Some("2032-05-10T04:00:00Z"),
    )
    .await;
    insert_attendance(&db, s_before, "IST1", "2032-05-09T18:29:59Z").await;
    insert_attendance(&db, s_at, "IST2", "2032-05-09T18:30:00Z").await;
    insert_attendance(&db, s_exam, "IST3", "2032-05-10T18:29:59Z").await;

    let (status, d9) = client
        .get(
            &app,
            "/api/admin/dashboard/sessions-by-date?date=2032-05-09",
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let d9 = ids(&d9, "session_id");
    assert!(d9.contains(&s_before.to_string()));
    assert!(!d9.contains(&s_at.to_string()));

    let (_, d10) = client
        .get(
            &app,
            "/api/admin/dashboard/sessions-by-date?date=2032-05-10",
        )
        .await;
    let d10_ids = ids(&d10, "session_id");
    assert!(d10_ids.contains(&s_at.to_string()));
    assert!(
        d10_ids.contains(&s_exam.to_string()),
        "last second of the IST day dropped"
    );
    assert!(!d10_ids.contains(&s_before.to_string()));
    // The drill-down labels a scheduled session with the time it starts.
    let exam_row = d10
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["session_id"] == s_exam.to_string())
        .unwrap();
    assert_eq!(exam_row["time"], "2032-05-10T04:00:00Z");

    // The attendance series reports the same check-ins under their IST date.
    let (_, series) = client
        .get(&app, "/api/admin/dashboard/attendance-series?days=30")
        .await;
    let dates: Vec<(String, String)> = series
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["session"].as_str().unwrap().to_string(),
                v["date"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let date_of = |name: &str| {
        dates
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d.clone())
            .unwrap_or_default()
    };
    assert_eq!(date_of("ist-d-before"), "2032-05-09");
    assert_eq!(date_of("ist-d-at"), "2032-05-10");
    assert_eq!(date_of("ist-d-exam"), "2032-05-10");

    let (status, _) = client
        .get(
            &app,
            "/api/admin/dashboard/sessions-by-date?date=not-a-date",
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
