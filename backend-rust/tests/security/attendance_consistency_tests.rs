//! Roster edits that carry history with them (roll-number corrections),
//! the initial batch upload's validation, dashboard counts that only count
//! real check-ins, and check-ins to a session that has no location.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use hmac::{Hmac, KeyInit, Mac};
use serial_test::file_serial;
use sha2::Sha256;
use tower::ServiceExt;
use uuid::Uuid;

use crate::exam_session_flow_tests::{
    create_test_app, seed_admin, seed_location_and_batch, unique, Client,
};

const PW: &str = "correct-horse-battery-staple";

async fn super_client(app: &axum::Router, db: &sqlx::PgPool) -> (Client, Uuid) {
    let u = unique("attcon-super");
    let id = seed_admin(db, &u, &format!("{u}@example.com"), PW, "super_admin").await;
    (Client::login(app, &u, PW).await, id)
}

async fn insert_attendance(db: &sqlx::PgPool, session_id: Uuid, roll: &str, status: &str) {
    sqlx::query(
        "INSERT INTO attendances ( \
            id, session_id, student_name, roll_number, photo_url, photo_public_id, \
            student_latitude, student_longitude, distance_from_location, verified, \
            source, status, captured_at \
         ) VALUES ($1, $2, 'Test Student', $3, 'https://example.com/p.jpg', '', 12.97, 77.59, 0.0, true, \
            'self_submitted', $4, now())",
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(roll.to_uppercase())
    .bind(status)
    .execute(db)
    .await
    .unwrap();
}

async fn create_session(client: &Client, app: &axum::Router, location: Uuid, batch: Uuid) -> Uuid {
    let (status, s) = client
        .mutate(
            app,
            "POST",
            "/api/admin/sessions",
            serde_json::json!({
                "locationId": location.to_string(),
                "batchId": batch.to_string(),
                "durationMinutes": 30,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{s:?}");
    Uuid::parse_str(s["_id"].as_str().unwrap()).unwrap()
}

async fn student_id_for(db: &sqlx::PgPool, batch: Uuid, roll: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM students WHERE batch_id = $1 AND upper(roll_number) = upper($2)",
    )
    .bind(batch)
    .bind(roll)
    .fetch_one(db)
    .await
    .unwrap()
}

fn student(name: &str, roll: &str) -> serde_json::Value {
    serde_json::json!({ "name": name, "rollNumber": roll })
}

/// F6: fixing a roll-number typo moves the student's past check-ins (and
/// passkey) with them, so earlier sessions don't flip to absent/off-roster.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn roll_number_correction_carries_attendance_and_passkey() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let typo = format!("TY{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    let fixed = format!("OK{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, &typo).await;
    let s1 = create_session(&client, &app, location_id, batch_id).await;
    let s2 = create_session(&client, &app, location_id, batch_id).await;
    insert_attendance(&db, s1, &typo, "present").await;
    insert_attendance(&db, s2, &typo, "present").await;
    sqlx::query(
        "INSERT INTO webauthn_credentials (id, student_id, credential_id, passkey, enrolled_at) \
         VALUES ($1, $2, $3, $4, now())",
    )
    .bind(Uuid::new_v4())
    .bind(&typo)
    .bind(format!("cred-{}", Uuid::new_v4()))
    .bind(vec![1u8, 2, 3])
    .execute(&db)
    .await
    .unwrap();
    let sid = student_id_for(&db, batch_id, &typo).await;

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/batches/{batch_id}/students/{sid}"),
            student("Test Student", &fixed),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    for s in [s1, s2] {
        let rolls: Vec<String> =
            sqlx::query_scalar("SELECT roll_number FROM attendances WHERE session_id = $1")
                .bind(s)
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            rolls,
            vec![fixed.clone()],
            "check-in must follow the correction"
        );
    }
    // The student's per-session history still shows them present.
    let (status, detail) = client
        .get(
            &app,
            &format!("/api/admin/batches/{batch_id}/students/{sid}/sessions"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{detail:?}");
    assert!(
        !detail.to_string().contains("absent"),
        "history must not flip to absent: {detail:?}"
    );
    let passkeys: Vec<String> = sqlx::query_scalar(
        "SELECT student_id FROM webauthn_credentials WHERE upper(student_id) IN ($1, $2)",
    )
    .bind(&typo)
    .bind(&fixed)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(passkeys, vec![fixed.clone()]);

    // A casing/whitespace-only change touches nothing.
    let (status, _) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/batches/{batch_id}/students/{sid}"),
            student("Test Student", &format!(" {} ", fixed.to_lowercase())),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND roll_number = $2",
    )
    .bind(s1)
    .bind(&fixed)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

/// F6: if a session already has check-ins under BOTH the old and the new roll
/// the edit is refused (409) and nothing changes.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn roll_number_correction_conflicting_with_existing_checkins_is_409() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let a = format!("CA{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    let b = format!("CB{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, &a).await;
    let s1 = create_session(&client, &app, location_id, batch_id).await;
    insert_attendance(&db, s1, &a, "present").await;
    // `b` checked in off-roster to the same session.
    insert_attendance(&db, s1, &b, "present").await;
    let sid = student_id_for(&db, batch_id, &a).await;

    let (status, body) = client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/batches/{batch_id}/students/{sid}"),
            student("Test Student", &b),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body:?}");
    assert!(
        body["message"].as_str().unwrap().contains("both"),
        "{body:?}"
    );
    let stored: String = sqlx::query_scalar("SELECT roll_number FROM students WHERE id = $1")
        .bind(sid)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(stored, a, "the student row must be unchanged");
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND roll_number = $2",
    )
    .bind(s1)
    .bind(&a)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(n, 1, "attendance must be unchanged");
}

/// F7: the initial upload skips duplicate roll numbers (reported), rejects an
/// empty roster and a blank name, and flags a repeated batch name.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn batch_upload_validates_name_duplicates_and_emptiness() {
    let (app, db) = create_test_app().await;
    let (client, _) = super_client(&app, &db).await;
    let name = format!("Upload {}", &Uuid::new_v4().simple().to_string()[..8]);

    let csv = b"Roll No,Name,Email\nR1,Alice,a@x.test\n r1 ,Alice Again,\nR2,Bob,not-an-email\nR3,Cara,\n";
    let (status, body) = client
        .post_form_file(&app, "/api/admin/batches", &[("name", &name)], "r.csv", csv)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    // R1 imported, " r1 " skipped as a duplicate; Bob's bad email row skipped.
    assert_eq!(body["studentsImported"], 2, "{body:?}");
    assert_eq!(body["studentsSkipped"], 2, "{body:?}");
    assert_eq!(body["duplicateName"], false);
    let errs = body["errors"].to_string();
    assert!(errs.contains("Duplicate roll number"), "{errs}");
    assert!(errs.contains("not a valid email"), "{errs}");
    let batch_id: Uuid = body["batchId"].as_str().unwrap().parse().unwrap();
    let rolls: Vec<String> = sqlx::query_scalar(
        "SELECT roll_number FROM students WHERE batch_id = $1 ORDER BY position",
    )
    .bind(batch_id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(rolls, vec!["R1", "R3"]);

    // Same name again: allowed but flagged.
    let (status, body) = client
        .post_form_file(
            &app,
            "/api/admin/batches",
            &[("name", &name.to_uppercase())],
            "r.csv",
            b"Roll No,Name\nZ1,Zed\n",
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert_eq!(body["duplicateName"], true, "{body:?}");

    // Header-only file: no students.
    let (status, body) = client
        .post_form_file(
            &app,
            "/api/admin/batches",
            &[("name", "Empty")],
            "r.csv",
            b"Roll No,Name\n",
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    // Blank (present but whitespace) name.
    let (status, body) = client
        .post_form_file(
            &app,
            "/api/admin/batches",
            &[("name", "   ")],
            "r.csv",
            b"Roll No,Name\nZ1,Zed\n",
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    assert!(
        body["message"].as_str().unwrap().contains("name"),
        "{body:?}"
    );
    let created: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM batches WHERE name IN ('Empty', '   ')")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(created, 0, "rejected uploads must not leave a batch behind");
}

/// F10: mentor 'absent' marks are not check-ins — in the dashboard totals,
/// recent activity, series and by-date views, and in a session's
/// `attendanceCount`. Location-less (exam) sessions are included.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn dashboard_and_session_counts_only_count_present_checkins() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, "DB001").await;
    let session = create_session(&client, &app, location_id, batch_id).await;
    insert_attendance(&db, session, "DB001", "present").await;
    insert_attendance(&db, session, "DB002", "absent").await;
    insert_attendance(&db, session, "DB003", "absent").await;

    // A location-less exam-style session with one present check-in.
    let locless: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (id, batch_id, token_hash, token_prefix, description, created_by, is_active, expires_at, \
         rotation_count, totp_secret, created_at, monitoring_enabled, session_kind, starts_at, college_name) \
         VALUES ($1, $2, $3, 'pfx', 'LOCLESS-EXAM', $4, true, now() + interval '1 hour', 0, 'secret', now(), false, 'attendance', now(), 'C') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(batch_id)
    .bind(format!("hash-{}", Uuid::new_v4()))
    .bind(admin_id)
    .fetch_one(&db)
    .await
    .unwrap();
    insert_attendance(&db, locless, "DB004", "present").await;

    let (_, detail) = client
        .get(&app, &format!("/api/admin/sessions/{session}"))
        .await;
    assert_eq!(detail["attendanceCount"], 1, "{detail:?}");
    let (_, list) = client.get(&app, "/api/admin/sessions").await;
    let mine = list
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["_id"] == session.to_string())
        .unwrap();
    assert_eq!(mine["attendanceCount"], 1, "{mine:?}");

    // sessions-by-date reads the date as an IST day, so ask for today in IST (UTC's date is
    // a day behind for 5.5 hours every evening).
    let today = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
        .format("%Y-%m-%d")
        .to_string();
    let (status, by_date) = client
        .get(
            &app,
            &format!("/api/admin/dashboard/sessions-by-date?date={today}"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_date:?}");
    let rows = by_date.as_array().unwrap();
    let row = rows
        .iter()
        .find(|r| r["session_id"] == session.to_string())
        .unwrap();
    assert_eq!(row["count"], 1, "absent marks excluded: {row:?}");
    assert!(
        rows.iter().any(|r| r["session_id"] == locless.to_string()),
        "location-less sessions must appear: {rows:?}"
    );

    let (status, series) = client
        .get(&app, "/api/admin/dashboard/attendance-series")
        .await;
    assert_eq!(status, StatusCode::OK);
    let items = series.as_array().unwrap();
    assert!(
        items.iter().any(|i| i["session"] == "LOCLESS-EXAM"),
        "{items:?}"
    );

    let (status, recent) = client
        .get(&app, "/api/admin/dashboard/recent-activity")
        .await;
    assert_eq!(status, StatusCode::OK);
    let recent = recent.as_array().unwrap();
    assert!(
        !recent
            .iter()
            .any(|r| r["roll_number"] == "DB002" || r["roll_number"] == "DB003"),
        "absent marks must not be recent check-ins: {recent:?}"
    );
    assert!(recent.iter().any(|r| r["roll_number"] == "DB001"));

    // Headline total only counts present rows (other tests share this DB, so
    // compare against a direct count rather than a constant).
    let (status, dash) = client.get(&app, "/api/admin/dashboard").await;
    assert_eq!(status, StatusCode::OK, "{dash:?}");
    let present: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE status = 'present'")
            .fetch_one(&db)
            .await
            .unwrap();
    let total = dash["charts"]["integrityBreakdown"]["totalCheckins"]
        .as_i64()
        .unwrap_or_else(|| panic!("totalCheckins not found in {dash:?}"));
    assert_eq!(total, present, "{dash:?}");
}

fn captcha_id(secret: &str, answer: &str) -> String {
    let ts = chrono::Utc::now().timestamp_millis();
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{}:{}", answer.to_lowercase(), ts).as_bytes());
    format!("{ts}.{}", hex::encode(mac.finalize().into_bytes()))
}

/// F14: a student can check in to a session that has no location (an exam
/// session created without one) — recorded as location-unverified — while a
/// session WITH a location still enforces its geofence.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn checkin_to_a_locationless_session_is_accepted_unverified() {
    let (app, db) = create_test_app().await;
    let (client, admin_id) = super_client(&app, &db).await;
    let mentor_name = unique("attcon-mentor");
    let mentor_id = seed_admin(
        &db,
        &mentor_name,
        &format!("{mentor_name}@example.com"),
        PW,
        "admin",
    )
    .await;
    let roll = format!("LL{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase();
    let (location_id, batch_id) = seed_location_and_batch(&db, admin_id, &roll).await;

    let create_exam = |with_location: bool| {
        let mut body = serde_json::json!({
            "batchId": batch_id.to_string(),
            "assignedAdminIds": [mentor_id],
            "collegeName": "XYZ College",
            "startsAt": (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339(),
            "durationMinutes": 60,
            "shortlinkMode": "auto",
        });
        if with_location {
            body["locationId"] = serde_json::json!(location_id.to_string());
        }
        body
    };
    let (status, loc_less) = client
        .mutate(&app, "POST", "/api/admin/sessions", create_exam(false))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loc_less:?}");
    let (status, with_loc) = client
        .mutate(&app, "POST", "/api/admin/sessions", create_exam(true))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{with_loc:?}");

    let secret = attendance_geotag_backend::config::AppConfig::for_testing().jwt_secret;
    let submit = |code: String, lat: f64, lon: f64| {
        let app = app.clone();
        let secret = secret.clone();
        let roll = roll.clone();
        async move {
            let mut req = Request::builder()
                .method("POST")
                .uri(format!("/api/s/{code}/submit"))
                .header("content-type", "application/json")
                .header("user-agent", "Mozilla/5.0 (Linux; Android 13; Mobile)")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "studentName": "Test Student",
                        "rollNumber": roll,
                        "latitude": lat,
                        "longitude": lon,
                        "captchaAnswer": "abcde",
                        "captchaId": captcha_id(&secret, "abcde"),
                    }))
                    .unwrap(),
                ))
                .unwrap();
            req.extensions_mut()
                .insert(ConnectInfo(std::net::SocketAddr::from((
                    [203, 0, 113, 7],
                    4000,
                ))));
            let response = app.oneshot(req).await.unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default(),
            )
        }
    };

    // Far from the real location: the geofenced session is still evaluated
    // (outside -> recorded unverified, same as before); the location-less one
    // is accepted and ALSO unverified, with no distance.
    let (status, body) = submit(
        loc_less["shortCode"].as_str().unwrap().to_string(),
        0.0,
        0.0,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "location-less check-in: {body:?}"
    );
    let (verified, distance): (bool, f64) = sqlx::query_as(
        "SELECT a.verified, a.distance_from_location FROM attendances a \
         WHERE a.session_id = $1 AND a.roll_number = $2",
    )
    .bind(Uuid::parse_str(loc_less["_id"].as_str().unwrap()).unwrap())
    .bind(&roll)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!verified, "no location -> location-unverified");
    assert_eq!(distance, 0.0);

    let (status, body) = submit(
        with_loc["shortCode"].as_str().unwrap().to_string(),
        12.9716,
        77.5946,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    let verified: bool = sqlx::query_scalar(
        "SELECT verified FROM attendances WHERE session_id = $1 AND roll_number = $2",
    )
    .bind(Uuid::parse_str(with_loc["_id"].as_str().unwrap()).unwrap())
    .bind(&roll)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        verified,
        "inside the geofence of a located session stays verified"
    );
}
