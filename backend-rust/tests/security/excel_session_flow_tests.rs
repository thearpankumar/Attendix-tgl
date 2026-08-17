//! End-to-end behavioural tests for the Excel-upload bulk session creation
//! feature: dry-run parse -> commit -> mentor roster/manual-mark -> mentor-
//! format export -> bulk export -> mentor add/remove. Same harness pattern
//! as `exam_session_flow_tests.rs` (real HTTP over a real Postgres + Redis,
//! testcontainers) — duplicated rather than shared, matching how every
//! other `*_flow_tests.rs`/`route_auth_tests.rs` file in this directory is
//! already self-contained.

use axum::{
    body::Body,
    http::{header::SET_COOKIE, HeaderMap, Request, StatusCode},
};
use calamine::{open_workbook_from_rs, Reader, Xlsx};
use serial_test::file_serial;
use std::io::Cursor;
use std::sync::Arc;
use tower::ServiceExt;

use attendance_geotag_backend::{
    config::AppConfig,
    middleware::{DenyList, RateLimiter, SessionCache},
    models::{Admin, SystemConfig},
    routes,
    services::GpsHistoryService,
    AppState,
};
use tokio::sync::RwLock;

async fn create_test_app() -> (axum::Router, sqlx::PgPool) {
    let db = crate::test_db::get_test_database().await;
    let config = AppConfig::for_testing();

    let redis_env = crate::test_db::get_test_environment().await;
    let redis_client =
        Arc::new(redis::Client::open(redis_env.redis_uri()).expect("valid test redis URL"));

    let rate_limiter = Arc::new(RateLimiter::new(redis_client.clone()));
    let deny_list = Arc::new(DenyList::new(redis_client.clone()));
    let session_cache = Arc::new(SessionCache::new(redis_client.clone(), 300));
    let gps_history = Arc::new(GpsHistoryService::new(redis_client.clone()));
    let mut default_config = SystemConfig::default();
    default_config.rate_limits.registration_max_requests = 100_000;
    default_config.rate_limits.login_max_requests = 100_000;
    let system_config = Arc::new(RwLock::new(default_config));

    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::v2026_01_12())
        .load()
        .await;
    let storage = attendance_geotag_backend::storage::Storage::new(&aws_config, &config.storage)
        .unwrap_or_else(|_| panic!("Failed to initialize test storage"));

    let webauthn = Arc::new(
        attendance_geotag_backend::build_webauthn(&config).expect("valid test webauthn config"),
    );

    let state = Arc::new(AppState {
        config: config.clone(),
        db: db.clone(),
        redis: (*redis_client).clone(),
        rate_limiter,
        deny_list,
        session_cache,
        gps_history,
        start_time: std::time::Instant::now(),
        storage,
        http_client: reqwest::Client::new(),
        system_config,
        webauthn,
    });

    (routes::create_routes(state), db)
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(SET_COOKIE).iter().find_map(|v| {
        let s = v.to_str().ok()?;
        let (kv, _rest) = s.split_once(';').unwrap_or((s, ""));
        let (k, val) = kv.split_once('=')?;
        (k == name).then(|| val.to_string())
    })
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}))
}

async fn bytes_body(response: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec()
}

struct Client {
    admin_token: String,
    csrf_token: String,
}

impl Client {
    async fn login(app: &axum::Router, username: &str, password: &str) -> Self {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/admin/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &serde_json::json!({ "username": username, "password": password }),
                        )
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "login should succeed");
        let admin_token =
            extract_cookie(response.headers(), "admin_token").expect("login sets admin_token");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/profile")
                    .header("cookie", format!("admin_token={}", admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let csrf_token =
            extract_cookie(response.headers(), "csrf_token").expect("GET issues csrf_token");

        Self {
            admin_token,
            csrf_token,
        }
    }

    fn cookie_header(&self) -> String {
        format!(
            "admin_token={}; csrf_token={}",
            self.admin_token, self.csrf_token
        )
    }

    async fn get(&self, app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, json_body(response).await)
    }

    async fn get_bytes(&self, app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, bytes_body(response).await)
    }

    async fn mutate(
        &self,
        app: &axum::Router,
        method: &str,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .header("x-csrf-token", &self.csrf_token)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, json_body(response).await)
    }

    async fn mutate_bytes(
        &self,
        app: &axum::Router,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, Vec<u8>) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .header("x-csrf-token", &self.csrf_token)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, bytes_body(response).await)
    }

    /// Posts a hand-built `multipart/form-data` body — axum's `Multipart`
    /// extractor has no client-side counterpart, so this mirrors exactly
    /// what a browser's `FormData` + fetch would send for the Excel-upload
    /// dry-run parse endpoint.
    async fn post_multipart_csv(
        &self,
        app: &axum::Router,
        uri: &str,
        fields: &[(&str, &str)],
        file_field_name: &str,
        file_name: &str,
        file_bytes: &[u8],
    ) -> (StatusCode, serde_json::Value) {
        let boundary = "excel-session-test-boundary";
        let mut body = Vec::new();
        for (name, value) in fields {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
                )
                .as_bytes(),
            );
        }
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{file_field_name}\"; filename=\"{file_name}\"\r\nContent-Type: text/csv\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(file_bytes);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("cookie", self.cookie_header())
                    .header("x-csrf-token", &self.csrf_token)
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, json_body(response).await)
    }
}

async fn seed_admin(
    pool: &sqlx::PgPool,
    username: &str,
    email: &str,
    password: &str,
    role: &str,
) -> uuid::Uuid {
    let hashed = Admin::hash_password(password).unwrap();
    sqlx::query_scalar(
        "INSERT INTO admins (id, username, email, password, role, is_active, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, true, 0, now()) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(username)
    .bind(email)
    .bind(hashed)
    .bind(role)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn unique(prefix: &str) -> String {
    format!(
        "{}-{}",
        prefix,
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

/// Reads back an xlsx byte buffer's first sheet's rows as strings — used to
/// assert on the actual generated export content rather than just status
/// codes. `calamine` is already a normal dependency of the main crate (used
/// by the batch-roster importer), so it's available here too.
fn read_xlsx_sheet(bytes: &[u8], sheet_index: usize) -> Vec<Vec<String>> {
    let mut wb = open_workbook_from_rs::<Xlsx<_>, _>(Cursor::new(bytes.to_vec()))
        .expect("export must be a valid xlsx file");
    let range = wb
        .worksheet_range_at(sheet_index)
        .unwrap_or_else(|| panic!("sheet {sheet_index} must exist"))
        .expect("sheet must be readable");
    range
        .rows()
        .map(|row| row.iter().map(|c| c.to_string()).collect())
        .collect()
}

/// Query-string values (track/class/time-slot names) can contain spaces,
/// which must be percent-encoded to survive as a single URI segment.
fn enc(s: &str) -> String {
    s.replace(' ', "%20")
}

fn sheet_count(bytes: &[u8]) -> usize {
    let wb = open_workbook_from_rs::<Xlsx<_>, _>(Cursor::new(bytes.to_vec()))
        .expect("export must be a valid xlsx file");
    wb.sheet_names().len()
}

/// Full round trip: upload a small master-roster CSV (two distinct
/// track+class+session_time groups, one with a matched mentor email and one
/// with an unmatched one), commit both, then exercise everything the
/// mentor/export/mentor-management surface can do with the result —
/// roster load, manual marking, mentor-format export content, bulk export
/// sheet count, and add/remove mentors.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn excel_upload_parse_commit_and_full_session_lifecycle() {
    let (app, db) = create_test_app().await;

    let super_username = unique("excel-super");
    seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    let mentor_username = unique("excel-mentor");
    let mentor_email = format!("{}@example.com", mentor_username);
    let (_status, mentor_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor_username,
                "email": mentor_email,
                "password": "mentor-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor_id = mentor_body["_id"].as_str().unwrap().to_string();

    let roll1 = unique("R1").to_uppercase();
    let roll2 = unique("R2").to_uppercase();
    let roll3 = unique("R3").to_uppercase();
    let csv = format!(
        "student_name,registration_number,email,track,class,session_time,round,status,marked_by,marked_at,assigned_mentor\n\
         Alice Test,{roll1},alice@example.com,DSA,CLS 1,2:00 PM - 4:00 PM,Python,,,,{mentor_email}\n\
         Bob Test,{roll2},bob@example.com,DSA,CLS 1,2:00 PM - 4:00 PM,Java,,,,{mentor_email}\n\
         Carol Test,{roll3},carol@example.com,Cybersecurity,LH 2,4:00 PM - 6:00 PM,,,,,unmatched-{roll3}@example.com\n"
    );

    // ── Dry-run parse ──────────────────────────────────────────────
    let (status, parsed) = super_client
        .post_multipart_csv(
            &app,
            "/api/admin/excel-sessions/parse",
            &[
                ("sessionDate", "2020-01-01"),
                ("collegeName", "Test College"),
            ],
            "file",
            "roster.csv",
            csv.as_bytes(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "parse should succeed: {parsed:?}");
    assert_eq!(parsed["totalRows"], 3);
    assert_eq!(parsed["totalGroups"], 2);
    assert!(parsed["parseErrors"].as_array().unwrap().is_empty());

    let groups = parsed["groups"].as_array().unwrap().clone();
    let dsa_group = groups
        .iter()
        .find(|g| g["track"] == "DSA")
        .expect("DSA group must be present");
    assert_eq!(dsa_group["studentCount"], 2);
    assert_eq!(dsa_group["mentors"][0]["matchedAdminId"], mentor_id);
    assert!(dsa_group["unmatchedMentorEmails"]
        .as_array()
        .unwrap()
        .is_empty());

    let cyber_group = groups
        .iter()
        .find(|g| g["track"] == "Cybersecurity")
        .expect("Cybersecurity group must be present");
    assert_eq!(cyber_group["studentCount"], 1);
    assert!(cyber_group["mentors"].as_array().unwrap().is_empty());
    assert_eq!(
        cyber_group["unmatchedMentorEmails"][0],
        format!("unmatched-{roll3}@example.com").to_lowercase()
    );

    // ── Commit both groups (assigning the real mentor to both, as the
    //    review UI would do for the group whose email didn't match) ──
    let build_commit_group = |g: &serde_json::Value| {
        serde_json::json!({
            "groupIndex": g["groupIndex"],
            "track": g["track"],
            "classLabel": g["classLabel"],
            "sessionTimeRaw": g["sessionTimeRaw"],
            "startsAt": format!("2020-01-01T{}:00Z", g["startTime"].as_str().unwrap()),
            "durationMinutes": g["durationMinutes"],
            "collegeName": "Test College",
            "description": format!("{} — {}", g["track"], g["classLabel"]),
            "assignedAdminIds": [mentor_id.clone()],
            "rawMentorEmails": [mentor_email.clone()],
            "students": g["students"],
        })
    };
    let commit_payload = serde_json::json!({
        "sourceFilename": "roster.csv",
        "sessionDate": "2020-01-01",
        "groups": [build_commit_group(dsa_group), build_commit_group(cyber_group)],
    });
    let (status, commit_result) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/excel-sessions/commit",
            commit_payload,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "commit should succeed: {commit_result:?}"
    );
    assert_eq!(commit_result["createdCount"], 2);
    assert_eq!(commit_result["failedCount"], 0);

    let results = commit_result["results"].as_array().unwrap();
    let dsa_session_id = results
        .iter()
        .find(|r| r["groupIndex"] == dsa_group["groupIndex"])
        .unwrap()["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let cyber_session_id = results
        .iter()
        .find(|r| r["groupIndex"] == cyber_group["groupIndex"])
        .unwrap()["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    // ── The Batches list now shows both as tagged "session" batches ──
    let (status, batches) = super_client.get(&app, "/api/admin/batches").await;
    assert_eq!(status, StatusCode::OK);
    let batches = batches.as_array().unwrap();
    assert!(
        batches
            .iter()
            .filter(|b| b["type"] == "session"
                && (b["linkedSessionId"] == dsa_session_id
                    || b["linkedSessionId"] == cyber_session_id))
            .count()
            == 2,
        "both excel-generated batches should appear tagged 'session': {batches:?}"
    );

    // ── An excel/mentor-based session has no `locations` row, but the
    //    review UI's "class" column IS the location — the sessions list and
    //    detail endpoints must surface it as `locationName` instead of
    //    leaving it blank/"Unknown" ──
    let (status, sessions_list) = super_client.get(&app, "/api/admin/sessions").await;
    assert_eq!(status, StatusCode::OK);
    let dsa_list_entry = sessions_list
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["_id"] == dsa_session_id)
        .expect("DSA session must be in the list");
    assert_eq!(dsa_list_entry["locationName"], "CLS 1");
    assert!(dsa_list_entry["locationId"].is_null());

    let (status, dsa_detail) = super_client
        .get(&app, &format!("/api/admin/sessions/{dsa_session_id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dsa_detail["locationName"], "CLS 1");

    // ── Mentor sees the DSA session's roster and marks one present ──
    let mentor_client = Client::login(&app, &mentor_username, "mentor-password-123").await;
    let (status, roster) = mentor_client
        .get(
            &app,
            &format!("/api/admin/sessions/{dsa_session_id}/roster"),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "mentor should see roster: {roster:?}"
    );
    assert_eq!(roster["summary"]["total"], 2);
    assert_eq!(roster["summary"]["unmarked"], 2);

    let (status, mark_body) = mentor_client
        .mutate(
            &app,
            "POST",
            &format!("/api/admin/sessions/{dsa_session_id}/attendance/manual"),
            serde_json::json!({ "rollNumber": roll1, "status": "present" }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "manual mark should succeed: {mark_body:?}"
    );

    // ── Track/class/time-slot/marking-status filters on the sessions list —
    //    the "download by track/class/time-slot without missing any"
    //    workflow. At this point DSA has 1/2 marked present (partial,
    //    50%) and Cybersecurity has 0/1 marked (not_started, 0%). ──
    let (status, filtered) = super_client
        .get(&app, "/api/admin/sessions?tracks=DSA")
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered = filtered.as_array().unwrap();
    assert_eq!(
        filtered.len(),
        1,
        "tracks=DSA should match only the DSA session"
    );
    assert_eq!(filtered[0]["_id"], dsa_session_id);
    assert_eq!(filtered[0]["track"], "DSA");
    assert_eq!(filtered[0]["markingStatus"], "partial");
    assert_eq!(filtered[0]["rosterSize"], 2);
    assert_eq!(filtered[0]["markedCount"], 1);
    assert_eq!(filtered[0]["attendancePercent"], 50.0);

    let (status, filtered) = super_client
        .get(&app, "/api/admin/sessions?classLabels=LH%202")
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered = filtered.as_array().unwrap();
    assert_eq!(
        filtered.len(),
        1,
        "classLabels=LH 2 should match only the Cybersecurity session"
    );
    assert_eq!(filtered[0]["_id"], cyber_session_id);
    assert_eq!(filtered[0]["markingStatus"], "not_started");

    // classLabels is multi-select: comma-separated values must match either.
    let (status, filtered) = super_client
        .get(&app, "/api/admin/sessions?classLabels=CLS%201,LH%202")
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered_ids: std::collections::HashSet<String> = filtered
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        filtered_ids,
        std::collections::HashSet::from([dsa_session_id.clone(), cyber_session_id.clone()]),
        "classLabels=CLS 1,LH 2 should match both sessions"
    );

    let dsa_time_enc = enc(dsa_group["sessionTimeRaw"].as_str().unwrap());
    let (status, filtered) = super_client
        .get(
            &app,
            &format!("/api/admin/sessions?timeSlots={dsa_time_enc}"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered = filtered.as_array().unwrap();
    assert_eq!(
        filtered.len(),
        1,
        "the DSA time slot should match only the DSA session"
    );
    assert_eq!(filtered[0]["_id"], dsa_session_id);

    let (status, filtered) = super_client
        .get(&app, "/api/admin/sessions?markingStatus=complete")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        filtered.as_array().unwrap().is_empty(),
        "neither session is fully marked yet"
    );

    let (status, filtered) = super_client
        .get(&app, "/api/admin/sessions?markingStatus=partial")
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered = filtered.as_array().unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["_id"], dsa_session_id);

    let (status, filtered) = super_client
        .get(&app, &format!("/api/admin/sessions?search={roll1}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let filtered = filtered.as_array().unwrap();
    assert_eq!(
        filtered.len(),
        1,
        "searching a roll number should find only the session whose roster contains it"
    );
    assert_eq!(filtered[0]["_id"], dsa_session_id);

    // ── Global stats overview: totals ignore filters, everything else
    //    reflects the currently active ones ──
    let (status, overview) = super_client
        .get(&app, "/api/admin/sessions/stats-overview")
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "stats-overview should succeed: {overview:?}"
    );
    assert_eq!(overview["totalRecords"], 2);
    assert_eq!(overview["filteredRecords"], 2);
    assert_eq!(overview["present"], 1);
    assert_eq!(overview["absent"], 2);
    assert_eq!(overview["topSession"]["id"], dsa_session_id);
    assert_eq!(overview["bottomSession"]["id"], cyber_session_id);

    let (status, overview_dsa) = super_client
        .get(&app, "/api/admin/sessions/stats-overview?tracks=DSA")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview_dsa["totalRecords"], 2, "totals stay unfiltered");
    assert_eq!(overview_dsa["filteredRecords"], 1);
    assert_eq!(overview_dsa["present"], 1);
    assert_eq!(overview_dsa["absent"], 1);

    // ── Export-filtered: exports exactly what the filter matches, without
    //    the caller needing to know session ids up front ──
    let (status, filtered_bytes) = super_client
        .mutate_bytes(
            &app,
            "/api/admin/sessions/export-filtered",
            serde_json::json!({ "tracks": ["DSA"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sheet_count(&filtered_bytes),
        1,
        "export-filtered(tracks=DSA) must contain exactly the DSA session"
    );

    // ── Mentor-format export: columns + content match the confirmed
    //    attendance-data-*.csv sample shape exactly ──
    let (status, export_bytes) = super_client
        .get_bytes(
            &app,
            &format!("/api/admin/sessions/{dsa_session_id}/export"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let rows = read_xlsx_sheet(&export_bytes, 0);
    assert_eq!(
        rows[0],
        vec![
            "Student Name",
            "Registration Number",
            "Email",
            "Track",
            "Class",
            "Batch",
            "Round",
            "Status",
            "Marked By",
            "Marked At",
        ]
    );
    let marked_row = rows
        .iter()
        .find(|r| r[1] == roll1)
        .expect("marked student's row must be present");
    assert_eq!(marked_row[3], "DSA");
    assert_eq!(
        marked_row[5], "2:00 PM - 4:00 PM",
        "Batch column holds session_time"
    );
    assert_eq!(marked_row[7], "Present");
    assert_eq!(marked_row[8], mentor_email);
    let unmarked_row = rows.iter().find(|r| r[1] == roll2).unwrap();
    assert_eq!(unmarked_row[7], "", "unmarked student has a blank status");

    // ── Bulk export: one workbook, one sheet per session ──
    let (status, bulk_bytes) = super_client
        .mutate_bytes(
            &app,
            "/api/admin/sessions/export-bulk",
            serde_json::json!({ "sessionIds": [dsa_session_id, cyber_session_id] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sheet_count(&bulk_bytes),
        2,
        "bulk export must contain one sheet per session"
    );

    // ── Mentor add/remove ──
    let mentor2_username = unique("excel-mentor2");
    let (_status, mentor2_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/users",
            serde_json::json!({
                "username": mentor2_username,
                "email": format!("{}@example.com", mentor2_username),
                "password": "mentor2-password-123",
                "role": "admin",
            }),
        )
        .await;
    let mentor2_id = mentor2_body["_id"].as_str().unwrap().to_string();

    let (status, patch_body) = super_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{dsa_session_id}/mentors"),
            serde_json::json!({ "add": [mentor2_id.clone()], "remove": [] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "adding a mentor should succeed: {patch_body:?}"
    );
    let ids: std::collections::HashSet<String> = patch_body["assignedAdminIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        ids,
        std::collections::HashSet::from([mentor_id.clone(), mentor2_id.clone()])
    );

    let (status, patch_body) = super_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{dsa_session_id}/mentors"),
            serde_json::json!({ "add": [], "remove": [mentor_id.clone()] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "removing a mentor should succeed: {patch_body:?}"
    );
    assert_eq!(patch_body["assignedAdminIds"].as_array().unwrap().len(), 1);
    assert_eq!(patch_body["assignedAdminIds"][0], mentor2_id);

    // A mentor cannot edit their own session's mentor list (only the
    // super-admin who created it can) — `update_session_mentors` is scoped
    // by direct `created_by`, deliberately not `find_session_for_admin`.
    let mentor2_client = Client::login(&app, &mentor2_username, "mentor2-password-123").await;
    let (status, _) = mentor2_client
        .mutate(
            &app,
            "PATCH",
            &format!("/api/admin/sessions/{dsa_session_id}/mentors"),
            serde_json::json!({ "add": [], "remove": [] }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an assigned mentor must not be able to edit their own session's mentor list"
    );

    // ── Bulk delete: wrong password is rejected, correct password deletes
    //    both sessions in one request ──
    let (status, _) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions/delete-bulk",
            serde_json::json!({
                "sessionIds": [dsa_session_id, cyber_session_id],
                "password": "wrong-password",
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "bulk delete must reject an incorrect password"
    );

    let (status, delete_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/sessions/delete-bulk",
            serde_json::json!({
                "sessionIds": [dsa_session_id, cyber_session_id],
                "password": "super-password-123",
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "bulk delete should succeed: {delete_body:?}"
    );
    assert_eq!(delete_body["deletedCount"], 2);
    assert!(delete_body["failedIds"].as_array().unwrap().is_empty());

    let (status, sessions_after) = super_client.get(&app, "/api/admin/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sessions_after
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["_id"] != dsa_session_id && s["_id"] != cyber_session_id),
        "both sessions must be gone after bulk delete"
    );
}

/// Infinite-scroll paging on `GET /batches` and `GET /sessions`: fetching in
/// small pages must return every row exactly once with no gaps/duplicates
/// across pages, respect `limit`, and — critically for every caller that
/// hasn't been updated to paginate (e.g. the Sessions page's batch-picker
/// dropdowns) — omitting `limit` entirely must still return everything.
#[tokio::test]
#[file_serial(admin_bootstrap)]
async fn list_endpoints_support_scroll_paging_without_gaps_or_duplicates() {
    let (app, db) = create_test_app().await;

    let super_username = unique("paging-super");
    seed_admin(
        &db,
        &super_username,
        &format!("{}@example.com", super_username),
        "super-password-123",
        "super_admin",
    )
    .await;
    let super_client = Client::login(&app, &super_username, "super-password-123").await;

    // ── Batches: create 5 manual batches, then page through them 2 at a time ──
    let batch_names: Vec<String> = (0..5).map(|i| unique(&format!("batch-{i}"))).collect();
    for name in &batch_names {
        let (status, _) = super_client
            .post_multipart_csv(
                &app,
                "/api/admin/batches",
                &[("name", name.as_str()), ("description", "paging test")],
                "file",
                "roster.csv",
                b"name,roll_number\nStudent A,R1\n",
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "seed batch {name} must be created"
        );
    }

    let (status, unpaginated) = super_client.get(&app, "/api/admin/batches").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        unpaginated.as_array().unwrap().len(),
        5,
        "omitting limit/offset must still return every batch"
    );

    let mut seen_batch_ids = std::collections::HashSet::new();
    let mut offset = 0;
    loop {
        let (status, page) = super_client
            .get(&app, &format!("/api/admin/batches?limit=2&offset={offset}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let page = page.as_array().unwrap().clone();
        assert!(
            page.len() <= 2,
            "a page must never exceed the requested limit"
        );
        if page.is_empty() {
            break;
        }
        for row in &page {
            let id = row["_id"].as_str().unwrap().to_string();
            assert!(
                seen_batch_ids.insert(id),
                "the same batch must never appear on two different pages"
            );
        }
        offset += page.len();
    }
    assert_eq!(
        seen_batch_ids.len(),
        5,
        "paging through with limit=2 must eventually surface every batch exactly once"
    );

    // ── Sessions: create 5 normal (GPS) sessions, then page through them ──
    let (status, location_body) = super_client
        .mutate(
            &app,
            "POST",
            "/api/admin/locations",
            serde_json::json!({
                "name": unique("paging-location"),
                "latitude": 12.9,
                "longitude": 77.6,
                "radiusMeters": 100,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "seed location: {location_body:?}"
    );
    let location_id = location_body["_id"].as_str().unwrap().to_string();

    let mut seeded_session_ids = std::collections::HashSet::new();
    for _ in 0..5 {
        let (status, session_body) = super_client
            .mutate(
                &app,
                "POST",
                "/api/admin/sessions",
                serde_json::json!({
                    "locationId": location_id,
                    "durationMinutes": 30,
                    "shortlinkMode": "none",
                }),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "seed session: {session_body:?}"
        );
        seeded_session_ids.insert(session_body["_id"].as_str().unwrap().to_string());
    }

    let (status, unpaginated) = super_client.get(&app, "/api/admin/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        unpaginated.as_array().unwrap().len(),
        5,
        "omitting limit/offset must still return every session"
    );

    let mut seen_session_ids = std::collections::HashSet::new();
    let mut offset = 0;
    loop {
        let (status, page) = super_client
            .get(
                &app,
                &format!("/api/admin/sessions?limit=2&offset={offset}"),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let page = page.as_array().unwrap().clone();
        assert!(
            page.len() <= 2,
            "a page must never exceed the requested limit"
        );
        if page.is_empty() {
            break;
        }
        for row in &page {
            let id = row["_id"].as_str().unwrap().to_string();
            assert!(
                seen_session_ids.insert(id),
                "the same session must never appear on two different pages"
            );
        }
        offset += page.len();
    }
    assert_eq!(seen_session_ids, seeded_session_ids);
}
