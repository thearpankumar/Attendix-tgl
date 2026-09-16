// Batch attendance analytics: per-student attendance percentages (batch
// Details page's Student View tab) and a global cross-batch roll-number
// Lookup, plus Excel export for both. One shared aggregation core
// (`compute_student_attendance`) is used by every surface here so what
// counts as "present" / "the denominator" never drifts between them — the
// same shape as `session_records.rs`'s shared-core-plus-thin-handlers split.
//
// Percentage semantics (locked in with the product owner): percentage =
// present / total_sessions_in_range. A session with no attendance row at all
// ("unmarked" — see manual_attendance.rs::get_session_roster) counts against
// the percentage exactly like an explicit "absent" row; the per-session
// detail table still reports the true three-way present/absent/unmarked
// status for transparency, only the rollup collapses unmarked into "not
// present".
//
// Date-range filtering uses `sessions.created_at` (always set) rather than
// `starts_at` (null for ordinary self-checkin sessions, only set for
// scheduled/exam sessions) — see migration 0011's comment. The session date
// shown to the user in tables is `COALESCE(starts_at, created_at)`.

use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::{
        BATCH_STUDENTS_DEFAULT_PAGE_SIZE, BATCH_STUDENTS_MAX_PAGE_SIZE,
        STUDENT_LOOKUP_MAX_ROLL_NUMBERS, STUDENT_SESSIONS_DEFAULT_PAGE_SIZE,
        STUDENT_SESSIONS_MAX_PAGE_SIZE,
    },
    controllers::{
        batch::{normalize_header, read_raw_rows, ROLL_NUMBER_ALIASES, ROLL_NUMBER_WEAK_ALIASES},
        status_label, source_label,
    },
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::AttendanceStatus,
};

/// Present/total/percentage for an arbitrary set of student IDs, scoped
/// independently per each student's own `batch_id` and the given date range.
/// Used identically by the per-batch Student View tab (every ID shares one
/// batch_id) and the global roll-number Lookup (IDs may span many batches).
/// Exactly two queries, regardless of how many student IDs are passed in:
/// the denominator (sessions per batch touched) is computed once per batch
/// via GROUP BY, never recomputed per student.
async fn compute_student_attendance(
    pool: &sqlx::PgPool,
    student_ids: &[Uuid],
    date_from: Option<DateTime<Utc>>,
    date_to: Option<DateTime<Utc>>,
) -> Result<HashMap<Uuid, (i64, i64)>> {
    if student_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let denom_rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT batch_id, COUNT(*) AS total \
         FROM sessions \
         WHERE batch_id IN (SELECT DISTINCT batch_id FROM students WHERE id = ANY($1)) \
           AND ($2::timestamptz IS NULL OR created_at >= $2) \
           AND ($3::timestamptz IS NULL OR created_at <= $3) \
         GROUP BY batch_id",
    )
    .bind(student_ids)
    .bind(date_from)
    .bind(date_to)
    .fetch_all(pool)
    .await?;
    let total_by_batch: HashMap<Uuid, i64> = denom_rows.into_iter().collect();

    // roll_number is stored as-uploaded (mixed case) on `students` but
    // always uppercased on write into `attendances` (controllers/attendance.rs,
    // controllers/admin/manual_attendance.rs) — only the `students` side of
    // the join needs `upper()`, so the existing
    // idx_attendances_session_roll (session_id, roll_number) index stays usable.
    let numer_rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT st.id AS student_id, st.batch_id, \
                COUNT(a.id) FILTER (WHERE a.status = 'present') AS present_count \
         FROM students st \
         LEFT JOIN sessions ses ON ses.batch_id = st.batch_id \
             AND ($2::timestamptz IS NULL OR ses.created_at >= $2) \
             AND ($3::timestamptz IS NULL OR ses.created_at <= $3) \
         LEFT JOIN attendances a ON a.session_id = ses.id AND a.roll_number = upper(st.roll_number) \
         WHERE st.id = ANY($1) \
         GROUP BY st.id, st.batch_id",
    )
    .bind(student_ids)
    .bind(date_from)
    .bind(date_to)
    .fetch_all(pool)
    .await?;

    Ok(numer_rows
        .into_iter()
        .map(|(student_id, batch_id, present)| {
            let total = total_by_batch.get(&batch_id).copied().unwrap_or(0);
            (student_id, (present, total))
        })
        .collect())
}

fn percentage(present: i64, total: i64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (present as f64 / total as f64) * 100.0
    }
}

/// Ownership-scoped check that `batch_id` belongs to `auth` — shared by every
/// handler below that operates on one specific batch. Returns just the
/// batch's own id (already known) so callers don't have to re-thread it.
async fn assert_owns_batch(
    pool: &sqlx::PgPool,
    auth: &AuthenticatedAdmin,
    batch_id: Uuid,
) -> Result<()> {
    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM batches WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(pool)
    .await?;
    exists
        .map(|_| ())
        .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))
}

// ---------------------------------------------------------------------
// GET /batches/{id}/students
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStudentsQuery {
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStudentRow {
    pub student_id: String,
    pub name: String,
    pub roll_number: String,
    pub email: Option<String>,
    pub present: i64,
    pub total_sessions: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStudentsResponse {
    pub students: Vec<BatchStudentRow>,
    pub has_more: bool,
}

#[derive(sqlx::FromRow)]
struct RosterPageRow {
    id: Uuid,
    name: String,
    roll_number: String,
    email: Option<String>,
}

/// Paginated, searchable, attendance-summarized roster for a batch's Student
/// View tab. Two-step by design: (1) a cheap, indexed page of `students`
/// (bounded regardless of roster size), then (2) `compute_student_attendance`
/// for just that page's IDs — never the whole roster at once.
pub async fn get_batch_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Query(q): Query<BatchStudentsQuery>,
) -> Result<impl IntoResponse> {
    let batch_id =
        Uuid::parse_str(&id).map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;
    assert_owns_batch(&state.db, &auth, batch_id).await?;

    let limit = q
        .limit
        .unwrap_or(BATCH_STUDENTS_DEFAULT_PAGE_SIZE)
        .clamp(1, BATCH_STUDENTS_MAX_PAGE_SIZE);
    let offset = q.offset.unwrap_or(0).max(0);
    let search = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let page: Vec<RosterPageRow> = sqlx::query_as(
        "SELECT id, name, roll_number, email FROM students \
         WHERE batch_id = $1 \
           AND ($2::text IS NULL OR name ILIKE '%' || $2 || '%' OR roll_number ILIKE '%' || $2 || '%') \
         ORDER BY position \
         LIMIT $3 OFFSET $4",
    )
    .bind(batch_id)
    .bind(search)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let student_ids: Vec<Uuid> = page.iter().map(|r| r.id).collect();
    let attendance = compute_student_attendance(&state.db, &student_ids, q.date_from, q.date_to).await?;

    let has_more = page.len() as i64 == limit;
    let students = page
        .into_iter()
        .map(|r| {
            let (present, total) = attendance.get(&r.id).copied().unwrap_or((0, 0));
            BatchStudentRow {
                student_id: r.id.to_string(),
                name: r.name,
                roll_number: r.roll_number,
                email: r.email,
                present,
                total_sessions: total,
                percentage: percentage(present, total),
            }
        })
        .collect();

    Ok(Json(BatchStudentsResponse { students, has_more }))
}

// ---------------------------------------------------------------------
// GET /batches/{id}/students/{studentId}/sessions
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentSessionsQuery {
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentSessionRow {
    pub session_id: String,
    pub session_date: DateTime<Utc>,
    /// "present" | "absent" | "unmarked"
    pub status: &'static str,
    /// null | "self_submitted" | "manual"
    pub source: Option<&'static str>,
    pub marked_by_email: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentSessionsResponse {
    pub sessions: Vec<StudentSessionRow>,
    pub has_more: bool,
}

#[derive(sqlx::FromRow)]
struct SessionDetailDbRow {
    session_id: Uuid,
    session_date: DateTime<Utc>,
    status: Option<AttendanceStatus>,
    source: Option<crate::models::AttendanceSource>,
    marked_by_email: Option<String>,
}

/// A student's full per-session date-wise breakdown — the expandable row in
/// the Student View tab. `studentId` is verified to actually belong to
/// `batchId` (not just "does this admin own *a* batch with this student
/// somewhere") so a mismatched pair of URL params 404s instead of leaking
/// another batch's student.
pub async fn get_student_session_detail(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, student_id)): Path<(String, String)>,
    Query(q): Query<StudentSessionsQuery>,
) -> Result<impl IntoResponse> {
    let batch_id =
        Uuid::parse_str(&id).map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;
    let student_id = Uuid::parse_str(&student_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid student ID: {}", e)))?;

    let roll_number: Option<String> = sqlx::query_scalar(
        "SELECT st.roll_number FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE st.id = $1 AND st.batch_id = $2 AND ($3 = 'super_admin' OR b.created_by = $4)",
    )
    .bind(student_id)
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?;
    let roll_number =
        roll_number.ok_or_else(|| AppError::NotFound("Student not found in this batch".to_string()))?;

    let limit = q
        .limit
        .unwrap_or(STUDENT_SESSIONS_DEFAULT_PAGE_SIZE)
        .clamp(1, STUDENT_SESSIONS_MAX_PAGE_SIZE);
    let offset = q.offset.unwrap_or(0).max(0);

    let rows: Vec<SessionDetailDbRow> = sqlx::query_as(
        "SELECT ses.id AS session_id, COALESCE(ses.starts_at, ses.created_at) AS session_date, \
                a.status, a.source, adm.email AS marked_by_email \
         FROM sessions ses \
         LEFT JOIN attendances a ON a.session_id = ses.id AND a.roll_number = upper($5) \
         LEFT JOIN admins adm ON adm.id = a.marked_by_admin_id \
         WHERE ses.batch_id = $1 \
           AND ($2::timestamptz IS NULL OR ses.created_at >= $2) \
           AND ($3::timestamptz IS NULL OR ses.created_at <= $3) \
         ORDER BY ses.created_at DESC \
         LIMIT $4 OFFSET $6",
    )
    .bind(batch_id)
    .bind(q.date_from)
    .bind(q.date_to)
    .bind(limit)
    .bind(&roll_number)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let has_more = rows.len() as i64 == limit;
    let sessions = rows
        .into_iter()
        .map(|r| StudentSessionRow {
            session_id: r.session_id.to_string(),
            session_date: r.session_date,
            status: r.status.map(status_label).unwrap_or("unmarked"),
            source: r.source.map(source_label),
            marked_by_email: r.marked_by_email,
        })
        .collect();

    Ok(Json(StudentSessionsResponse { sessions, has_more }))
}

// ---------------------------------------------------------------------
// GET /students — the global, direct "Student View": every student across
// every batch this admin can see, browsable without first knowing a roll
// number (unlike /students/lookup, which requires input to resolve
// against). Same two-step pattern as get_batch_students: a cheap, indexed
// page of `students` first, then compute_student_attendance for just that
// page's IDs — never the whole cross-batch roster at once.
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllStudentsQuery {
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllStudentRow {
    pub student_id: String,
    pub name: String,
    pub roll_number: String,
    pub email: Option<String>,
    pub batch_id: String,
    pub batch_name: String,
    pub present: i64,
    pub total_sessions: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllStudentsResponse {
    pub students: Vec<AllStudentRow>,
    pub has_more: bool,
}

#[derive(sqlx::FromRow)]
struct AllStudentsPageRow {
    id: Uuid,
    name: String,
    roll_number: String,
    email: Option<String>,
    batch_id: Uuid,
    batch_name: String,
}

pub async fn get_all_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(q): Query<AllStudentsQuery>,
) -> Result<impl IntoResponse> {
    let limit = q
        .limit
        .unwrap_or(BATCH_STUDENTS_DEFAULT_PAGE_SIZE)
        .clamp(1, BATCH_STUDENTS_MAX_PAGE_SIZE);
    let offset = q.offset.unwrap_or(0).max(0);
    let search = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let page: Vec<AllStudentsPageRow> = sqlx::query_as(
        "SELECT st.id, st.name, st.roll_number, st.email, st.batch_id, b.name AS batch_name \
         FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE ($1 = 'super_admin' OR b.created_by = $2) \
           AND ($3::text IS NULL OR st.name ILIKE '%' || $3 || '%' \
                OR st.roll_number ILIKE '%' || $3 || '%' OR b.name ILIKE '%' || $3 || '%') \
         ORDER BY st.name, st.id \
         LIMIT $4 OFFSET $5",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .bind(search)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let student_ids: Vec<Uuid> = page.iter().map(|r| r.id).collect();
    let attendance = compute_student_attendance(&state.db, &student_ids, q.date_from, q.date_to).await?;

    let has_more = page.len() as i64 == limit;
    let students = page
        .into_iter()
        .map(|r| {
            let (present, total) = attendance.get(&r.id).copied().unwrap_or((0, 0));
            AllStudentRow {
                student_id: r.id.to_string(),
                name: r.name,
                roll_number: r.roll_number,
                email: r.email,
                batch_id: r.batch_id.to_string(),
                batch_name: r.batch_name,
                present,
                total_sessions: total,
                percentage: percentage(present, total),
            }
        })
        .collect();

    Ok(Json(AllStudentsResponse { students, has_more }))
}

// ---------------------------------------------------------------------
// POST /batches/{id}/export
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    Selected,
    All,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportRequest {
    pub scope: ExportScope,
    #[serde(default)]
    pub student_ids: Vec<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow, Clone)]
struct ExportRosterRow {
    id: Uuid,
    name: String,
    roll_number: String,
    email: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ExportDetailRow {
    // Roster order, carried through purely to sort in Rust after fetch (see
    // export_batch_students) — not written to the workbook.
    position: i32,
    name: String,
    roll_number: String,
    session_date: DateTime<Utc>,
    status: Option<AttendanceStatus>,
}

/// Exports either the whole batch or an explicit checkbox-selected subset —
/// an explicit `scope` enum rather than "empty studentIds means everything",
/// so a frontend bug that fails to populate a selection can never silently
/// export the wrong (much larger) thing.
pub async fn export_batch_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<BatchExportRequest>,
) -> Result<impl IntoResponse> {
    let batch_id =
        Uuid::parse_str(&id).map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;
    assert_owns_batch(&state.db, &auth, batch_id).await?;

    let roster: Vec<ExportRosterRow> = match payload.scope {
        ExportScope::All => {
            sqlx::query_as(
                "SELECT id, name, roll_number, email FROM students WHERE batch_id = $1 ORDER BY position",
            )
            .bind(batch_id)
            .fetch_all(&state.db)
            .await?
        }
        ExportScope::Selected => {
            let ids: Vec<Uuid> = payload
                .student_ids
                .iter()
                .filter_map(|s| Uuid::parse_str(s).ok())
                .collect();
            if ids.is_empty() {
                return Err(AppError::BadRequest(
                    "studentIds is required when scope is 'selected'".to_string(),
                ));
            }
            // batch_id = $2 re-verifies every id actually belongs to this
            // batch — a manipulated id from another batch is silently
            // dropped rather than leaking that student's data here.
            sqlx::query_as(
                "SELECT id, name, roll_number, email FROM students \
                 WHERE id = ANY($1) AND batch_id = $2 ORDER BY position",
            )
            .bind(&ids)
            .bind(batch_id)
            .fetch_all(&state.db)
            .await?
        }
    };

    if roster.is_empty() {
        return Err(AppError::NotFound(
            "No matching students found for this batch".to_string(),
        ));
    }

    let student_ids: Vec<Uuid> = roster.iter().map(|r| r.id).collect();
    let attendance =
        compute_student_attendance(&state.db, &student_ids, payload.date_from, payload.date_to).await?;

    // No ORDER BY here — sorted in Rust below instead. On a large batch this
    // query's result set can be tens of thousands of rows; asking Postgres
    // to sort that (st.position, ses.created_at DESC) tends to exceed the
    // default work_mem and spill to an on-disk external merge sort, which
    // measured as the majority of this endpoint's latency on a seeded
    // 400-student x 200-session batch (~230ms of a ~240ms query). Sorting
    // the same rows in Rust after fetch is sub-millisecond at that size and
    // avoids the disk spill entirely.
    let mut details: Vec<ExportDetailRow> = sqlx::query_as(
        "SELECT st.position, st.name, st.roll_number, COALESCE(ses.starts_at, ses.created_at) AS session_date, a.status \
         FROM students st \
         JOIN sessions ses ON ses.batch_id = st.batch_id \
             AND ($3::timestamptz IS NULL OR ses.created_at >= $3) \
             AND ($4::timestamptz IS NULL OR ses.created_at <= $4) \
         LEFT JOIN attendances a ON a.session_id = ses.id AND a.roll_number = upper(st.roll_number) \
         WHERE st.id = ANY($1) AND st.batch_id = $2",
    )
    .bind(&student_ids)
    .bind(batch_id)
    .bind(payload.date_from)
    .bind(payload.date_to)
    .fetch_all(&state.db)
    .await?;
    details.sort_by(|a, b| a.position.cmp(&b.position).then(b.session_date.cmp(&a.session_date)));

    let excel_data = build_batch_export_workbook(&roster, &attendance, &details)?;

    let filename = format!(
        "Batch_Attendance_{}_{}.xlsx",
        batch_id,
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    Ok(xlsx_attachment(filename, excel_data))
}

fn xlsx_attachment(filename: String, data: Vec<u8>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        data,
    )
}

/// Single-batch export workbook ("Export Selected"/"Export Full Batch" on
/// the Batch Details page). Long format (one row per student in "Summary",
/// one row per student x session in "Details") rather than a wide
/// session-dates-as-columns pivot — a batch can hold multiple sessions on
/// the same calendar day, so "date as column header" isn't a valid key, and
/// hundreds of session columns is worse to read than the equivalent rows (an
/// admin can pivot this in Excel in seconds; the reverse isn't true). The
/// cross-batch global Lookup export has its own `build_lookup_export_workbook`
/// below, which adds a Batch Name column plus two Lookup-only sheets.
fn build_batch_export_workbook(
    roster: &[ExportRosterRow],
    attendance: &HashMap<Uuid, (i64, i64)>,
    details: &[ExportDetailRow],
) -> Result<Vec<u8>> {
    use rust_xlsxwriter::Workbook;
    let xl_err = |e: rust_xlsxwriter::XlsxError| AppError::Internal(format!("Excel error: {}", e));

    let mut workbook = Workbook::new();

    let summary = workbook.add_worksheet();
    summary.set_name("Summary").map_err(xl_err)?;
    for (col, header) in ["Name", "Roll Number", "Email", "Present", "Total Sessions", "Percentage"]
        .iter()
        .enumerate()
    {
        summary.write_string(0, col as u16, *header).map_err(xl_err)?;
    }
    for (i, s) in roster.iter().enumerate() {
        let row = (i + 1) as u32;
        let (present, total) = attendance.get(&s.id).copied().unwrap_or((0, 0));
        summary.write_string(row, 0, &s.name).map_err(xl_err)?;
        summary.write_string(row, 1, &s.roll_number).map_err(xl_err)?;
        summary.write_string(row, 2, s.email.as_deref().unwrap_or("")).map_err(xl_err)?;
        summary.write_number(row, 3, present as f64).map_err(xl_err)?;
        summary.write_number(row, 4, total as f64).map_err(xl_err)?;
        summary.write_number(row, 5, percentage(present, total)).map_err(xl_err)?;
    }

    let sheet = workbook.add_worksheet();
    sheet.set_name("Details").map_err(xl_err)?;
    for (col, header) in ["Student Name", "Roll Number", "Session Date", "Status"]
        .iter()
        .enumerate()
    {
        sheet.write_string(0, col as u16, *header).map_err(xl_err)?;
    }
    for (i, d) in details.iter().enumerate() {
        let row = (i + 1) as u32;
        sheet.write_string(row, 0, &d.name).map_err(xl_err)?;
        sheet.write_string(row, 1, &d.roll_number).map_err(xl_err)?;
        sheet.write_string(row, 2, d.session_date.to_rfc3339()).map_err(xl_err)?;
        let status_str = d.status.map(status_label).unwrap_or("unmarked");
        sheet.write_string(row, 3, status_str).map_err(xl_err)?;
    }

    workbook.save_to_buffer().map_err(xl_err)
}

// ---------------------------------------------------------------------
// Global roll-number Lookup: POST /students/lookup, /students/lookup/file,
// /students/lookup/export
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentLookupRequest {
    pub roll_numbers: Vec<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LookupMatch {
    pub roll_number_queried: String,
    pub student_id: String,
    pub batch_id: String,
    pub batch_name: String,
    pub name: String,
    pub email: Option<String>,
    pub present: i64,
    pub total_sessions: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollNumberAggregate {
    pub roll_number: String,
    pub total_present: i64,
    pub total_sessions: i64,
    pub percentage: f64,
    pub matched_batch_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentLookupResponse {
    pub matches: Vec<LookupMatch>,
    pub aggregates: Vec<RollNumberAggregate>,
    pub not_found: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct LookupDbRow {
    id: Uuid,
    name: String,
    email: Option<String>,
    roll_number: String,
    batch_id: Uuid,
    batch_name: String,
}

/// One row of the Lookup export's "Details" sheet (student x session).
#[derive(sqlx::FromRow)]
struct LookupDetailDbRow {
    name: String,
    roll_number: String,
    batch_name: String,
    session_date: DateTime<Utc>,
    status: Option<AttendanceStatus>,
}

/// Resolves a list of pasted/uploaded roll numbers to every matching student
/// row across every batch — role-scoped in SQL (not filtered after the fact
/// in Rust), since without that a mentor pasting roll numbers could see
/// another mentor's students, which would be a cross-tenant data leak, not a
/// cosmetic bug.
async fn resolve_roll_number_lookup(
    pool: &sqlx::PgPool,
    auth: &AuthenticatedAdmin,
    roll_numbers: &[String],
    date_from: Option<DateTime<Utc>>,
    date_to: Option<DateTime<Utc>>,
) -> Result<StudentLookupResponse> {
    let queried: Vec<String> = roll_numbers
        .iter()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect();
    if queried.is_empty() {
        return Ok(StudentLookupResponse {
            matches: vec![],
            aggregates: vec![],
            not_found: vec![],
        });
    }
    if queried.len() > STUDENT_LOOKUP_MAX_ROLL_NUMBERS {
        return Err(AppError::BadRequest(format!(
            "At most {} roll numbers may be looked up at once",
            STUDENT_LOOKUP_MAX_ROLL_NUMBERS
        )));
    }
    let queried_upper: Vec<String> = queried.iter().map(|r| r.to_uppercase()).collect();

    let rows: Vec<LookupDbRow> = sqlx::query_as(
        "SELECT st.id, st.name, st.email, st.roll_number, st.batch_id, b.name AS batch_name \
         FROM students st JOIN batches b ON b.id = st.batch_id \
         WHERE upper(st.roll_number) = ANY($1) AND ($2 = 'super_admin' OR b.created_by = $3)",
    )
    .bind(&queried_upper)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(pool)
    .await?;

    let student_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let attendance = compute_student_attendance(pool, &student_ids, date_from, date_to).await?;

    let mut found_upper: std::collections::HashSet<String> = std::collections::HashSet::new();
    let matches: Vec<LookupMatch> = rows
        .iter()
        .map(|r| {
            found_upper.insert(r.roll_number.to_uppercase());
            let (present, total) = attendance.get(&r.id).copied().unwrap_or((0, 0));
            LookupMatch {
                roll_number_queried: r.roll_number.clone(),
                student_id: r.id.to_string(),
                batch_id: r.batch_id.to_string(),
                batch_name: r.batch_name.clone(),
                name: r.name.clone(),
                email: r.email.clone(),
                present,
                total_sessions: total,
                percentage: percentage(present, total),
            }
        })
        .collect();

    let not_found: Vec<String> = queried
        .into_iter()
        .zip(queried_upper.iter())
        .filter(|(_, upper)| !found_upper.contains(*upper))
        .map(|(original, _)| original)
        .collect();

    // Aggregate per distinct uppercased roll number across every batch it
    // matched — a roll number is only unique within a batch, so the same
    // input can legitimately resolve to several different students.
    let mut by_roll: HashMap<String, Vec<&LookupMatch>> = HashMap::new();
    for m in &matches {
        by_roll
            .entry(m.roll_number_queried.to_uppercase())
            .or_default()
            .push(m);
    }
    let mut aggregates: Vec<RollNumberAggregate> = by_roll
        .into_iter()
        .map(|(roll_number, ms)| {
            let total_present: i64 = ms.iter().map(|m| m.present).sum();
            let total_sessions: i64 = ms.iter().map(|m| m.total_sessions).sum();
            RollNumberAggregate {
                roll_number,
                total_present,
                total_sessions,
                percentage: percentage(total_present, total_sessions),
                matched_batch_count: ms.len(),
            }
        })
        .collect();
    aggregates.sort_by(|a, b| a.roll_number.cmp(&b.roll_number));

    Ok(StudentLookupResponse {
        matches,
        aggregates,
        not_found,
    })
}

pub async fn lookup_students_by_roll(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<StudentLookupRequest>,
) -> Result<impl IntoResponse> {
    let result = resolve_roll_number_lookup(
        &state.db,
        &auth,
        &payload.roll_numbers,
        payload.date_from,
        payload.date_to,
    )
    .await?;
    Ok(Json(result))
}

/// Extracts just the roll-number column from an uploaded lookup file, reusing
/// the same alias/case/punctuation-insensitive header detection as the
/// batch-roster importer (`batch::parse_excel`) via the shared
/// `ROLL_NUMBER_ALIASES` list — but only ever looks for one column, since a
/// lookup file's other columns (if any) are irrelevant: everything else is
/// resolved from `students` once the roll number matches. Deliberately does
/// NOT call `parse_excel` itself, which requires both a name AND a roll
/// number per row and would silently drop a bare single-column roll-number
/// file with no header.
fn parse_roll_numbers_from_file(data: &[u8]) -> Result<Vec<String>> {
    let raw_rows = read_raw_rows(data)?;
    if raw_rows.is_empty() {
        return Ok(vec![]);
    }

    // Two passes, not one combined membership check: a lookup file only ever
    // has this one column to identify, so unlike `batch::parse_excel` (which
    // cross-checks against name/email/college columns too) there's nothing
    // else to disambiguate an accidental match. Scanning for an unambiguous
    // alias (ROLL_NUMBER_ALIASES) first, and only falling back to a generic
    // one (ROLL_NUMBER_WEAK_ALIASES — "S.No", "ID", etc.) if no column
    // matched, stops a genuine "Roll Number" column from being shadowed by
    // an unrelated leading "S.No" index column just because it happens to
    // sit further left.
    let normalized_headers: Vec<String> = raw_rows[0].iter().map(|c| normalize_header(c)).collect();
    let mut roll_col = normalized_headers
        .iter()
        .position(|h| ROLL_NUMBER_ALIASES.contains(&h.as_str()));
    if roll_col.is_none() {
        roll_col = normalized_headers
            .iter()
            .position(|h| ROLL_NUMBER_WEAK_ALIASES.contains(&h.as_str()));
    }
    let start = if roll_col.is_some() { 1 } else { 0 };

    let roll_col = roll_col
        .or_else(|| {
            raw_rows.get(start).and_then(|row| {
                if row.len() <= 1 {
                    Some(0)
                } else {
                    row.iter().position(|c| c.chars().any(|ch| ch.is_ascii_digit()))
                }
            })
        })
        .unwrap_or(0);

    Ok(raw_rows
        .iter()
        .skip(start)
        .filter_map(|row| row.get(roll_col))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

pub async fn lookup_students_from_file(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut date_from: Option<DateTime<Utc>> = None;
    let mut date_to: Option<DateTime<Utc>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        match field.name().unwrap_or("") {
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
                        .to_vec(),
                );
            }
            "dateFrom" => {
                if let Ok(text) = field.text().await {
                    date_from = DateTime::parse_from_rfc3339(&text).ok().map(|d| d.with_timezone(&Utc));
                }
            }
            "dateTo" => {
                if let Ok(text) = field.text().await {
                    date_to = DateTime::parse_from_rfc3339(&text).ok().map(|d| d.with_timezone(&Utc));
                }
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| AppError::BadRequest("No file uploaded".to_string()))?;
    let roll_numbers = parse_roll_numbers_from_file(&file_data)?;

    let result = resolve_roll_number_lookup(&state.db, &auth, &roll_numbers, date_from, date_to).await?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupExportRequest {
    pub student_ids: Vec<String>,
    #[serde(default)]
    pub not_found: Vec<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

/// Exports the already-resolved matches from a prior `/students/lookup`
/// call — never re-parses the original paste/file — and recomputes
/// present/total server-side at export time rather than trusting
/// client-cached percentages (avoids staleness between "I searched" and "I
/// clicked export", and a client can't fabricate numbers into the file).
pub async fn export_student_lookup(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<LookupExportRequest>,
) -> Result<impl IntoResponse> {
    let ids: Vec<Uuid> = payload
        .student_ids
        .iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();
    if ids.is_empty() {
        return Err(AppError::BadRequest("studentIds must not be empty".to_string()));
    }

    let rows: Vec<LookupDbRow> = sqlx::query_as(
        "SELECT st.id, st.name, st.email, st.roll_number, st.batch_id, b.name AS batch_name \
         FROM students st JOIN batches b ON b.id = st.batch_id \
         WHERE st.id = ANY($1) AND ($2 = 'super_admin' OR b.created_by = $3)",
    )
    .bind(&ids)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    if rows.is_empty() {
        return Err(AppError::NotFound("No matching students found".to_string()));
    }

    let student_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let attendance =
        compute_student_attendance(&state.db, &student_ids, payload.date_from, payload.date_to).await?;

    let roster: Vec<ExportRosterRow> = rows
        .iter()
        .map(|r| ExportRosterRow {
            id: r.id,
            name: r.name.clone(),
            roll_number: r.roll_number.clone(),
            email: r.email.clone(),
        })
        .collect();
    let batch_names: HashMap<Uuid, String> =
        rows.iter().map(|r| (r.id, r.batch_name.clone())).collect();
    let batch_ids: Vec<Uuid> = rows.iter().map(|r| r.batch_id).collect();

    // No ORDER BY — sorted in Rust after fetch, same reasoning as
    // export_batch_students: avoids Postgres spilling a large sort to disk
    // (measured on a seeded batch to be the majority of a full-export's
    // latency) in exchange for a sub-millisecond in-memory sort.
    let mut detail_rows: Vec<LookupDetailDbRow> = sqlx::query_as(
        "SELECT st.name, st.roll_number, b.name AS batch_name, \
                COALESCE(ses.starts_at, ses.created_at) AS session_date, a.status \
         FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         JOIN sessions ses ON ses.batch_id = st.batch_id \
             AND ($3::timestamptz IS NULL OR ses.created_at >= $3) \
             AND ($4::timestamptz IS NULL OR ses.created_at <= $4) \
         LEFT JOIN attendances a ON a.session_id = ses.id AND a.roll_number = upper(st.roll_number) \
         WHERE st.id = ANY($1) AND st.batch_id = ANY($2)",
    )
    .bind(&student_ids)
    .bind(&batch_ids)
    .bind(payload.date_from)
    .bind(payload.date_to)
    .fetch_all(&state.db)
    .await?;
    detail_rows.sort_by(|a, b| a.name.cmp(&b.name).then(b.session_date.cmp(&a.session_date)));

    let excel_data = build_lookup_export_workbook(
        &roster,
        &attendance,
        &batch_names,
        &detail_rows,
        &payload.not_found,
    )?;

    let filename = format!(
        "Student_Lookup_Export_{}.xlsx",
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    Ok(xlsx_attachment(filename, excel_data))
}

/// Full workbook for the global Lookup export: the shared Summary/Details
/// sheets plus two Lookup-only sheets ("Not Found", "Aggregates").
fn build_lookup_export_workbook(
    roster: &[ExportRosterRow],
    attendance: &HashMap<Uuid, (i64, i64)>,
    batch_names: &HashMap<Uuid, String>,
    detail_rows: &[LookupDetailDbRow],
    not_found: &[String],
) -> Result<Vec<u8>> {
    use rust_xlsxwriter::Workbook;
    let xl_err = |e: rust_xlsxwriter::XlsxError| AppError::Internal(format!("Excel error: {}", e));

    let mut workbook = Workbook::new();

    let summary = workbook.add_worksheet();
    summary.set_name("Summary").map_err(xl_err)?;
    for (col, header) in ["Name", "Roll Number", "Email", "Batch Name", "Present", "Total Sessions", "Percentage"]
        .iter()
        .enumerate()
    {
        summary.write_string(0, col as u16, *header).map_err(xl_err)?;
    }
    for (i, s) in roster.iter().enumerate() {
        let row = (i + 1) as u32;
        let (present, total) = attendance.get(&s.id).copied().unwrap_or((0, 0));
        summary.write_string(row, 0, &s.name).map_err(xl_err)?;
        summary.write_string(row, 1, &s.roll_number).map_err(xl_err)?;
        summary.write_string(row, 2, s.email.as_deref().unwrap_or("")).map_err(xl_err)?;
        summary
            .write_string(row, 3, batch_names.get(&s.id).map(String::as_str).unwrap_or(""))
            .map_err(xl_err)?;
        summary.write_number(row, 4, present as f64).map_err(xl_err)?;
        summary.write_number(row, 5, total as f64).map_err(xl_err)?;
        summary.write_number(row, 6, percentage(present, total)).map_err(xl_err)?;
    }

    let details = workbook.add_worksheet();
    details.set_name("Details").map_err(xl_err)?;
    for (col, header) in ["Student Name", "Roll Number", "Batch Name", "Session Date", "Status"]
        .iter()
        .enumerate()
    {
        details.write_string(0, col as u16, *header).map_err(xl_err)?;
    }
    for (i, d) in detail_rows.iter().enumerate() {
        let row = (i + 1) as u32;
        details.write_string(row, 0, &d.name).map_err(xl_err)?;
        details.write_string(row, 1, &d.roll_number).map_err(xl_err)?;
        details.write_string(row, 2, &d.batch_name).map_err(xl_err)?;
        details.write_string(row, 3, d.session_date.to_rfc3339()).map_err(xl_err)?;
        details
            .write_string(row, 4, d.status.map(status_label).unwrap_or("unmarked"))
            .map_err(xl_err)?;
    }

    if !not_found.is_empty() {
        let nf = workbook.add_worksheet();
        nf.set_name("Not Found").map_err(xl_err)?;
        nf.write_string(0, 0, "Roll Number").map_err(xl_err)?;
        for (i, roll) in not_found.iter().enumerate() {
            nf.write_string((i + 1) as u32, 0, roll).map_err(xl_err)?;
        }
    }

    let mut by_roll: HashMap<String, (i64, i64, usize)> = HashMap::new();
    for s in roster {
        let (present, total) = attendance.get(&s.id).copied().unwrap_or((0, 0));
        let entry = by_roll.entry(s.roll_number.to_uppercase()).or_insert((0, 0, 0));
        entry.0 += present;
        entry.1 += total;
        entry.2 += 1;
    }
    let agg = workbook.add_worksheet();
    agg.set_name("Aggregates").map_err(xl_err)?;
    for (col, header) in ["Roll Number", "Total Present", "Total Sessions", "Percentage", "Matched Batches"]
        .iter()
        .enumerate()
    {
        agg.write_string(0, col as u16, *header).map_err(xl_err)?;
    }
    let mut agg_rows: Vec<(String, i64, i64, usize)> = by_roll
        .into_iter()
        .map(|(roll, (present, total, count))| (roll, present, total, count))
        .collect();
    agg_rows.sort_by(|a, b| a.0.cmp(&b.0));
    for (i, (roll, present, total, count)) in agg_rows.iter().enumerate() {
        let row = (i + 1) as u32;
        agg.write_string(row, 0, roll).map_err(xl_err)?;
        agg.write_number(row, 1, *present as f64).map_err(xl_err)?;
        agg.write_number(row, 2, *total as f64).map_err(xl_err)?;
        agg.write_number(row, 3, percentage(*present, *total)).map_err(xl_err)?;
        agg.write_number(row, 4, *count as f64).map_err(xl_err)?;
    }

    workbook.save_to_buffer().map_err(xl_err)
}

#[cfg(test)]
mod batch_analytics_tests {
    use super::*;

    #[test]
    fn test_percentage() {
        assert_eq!(percentage(0, 0), 0.0);
        assert_eq!(percentage(3, 4), 75.0);
        assert_eq!(percentage(0, 5), 0.0);
    }

    #[test]
    fn test_parse_roll_numbers_header_match() {
        let csv = b"Roll Number,Name\n21B91A0501,Alice\n21B91A0502,Bob\n";
        let rolls = parse_roll_numbers_from_file(csv).unwrap();
        assert_eq!(rolls, vec!["21B91A0501", "21B91A0502"]);
    }

    #[test]
    fn test_parse_roll_numbers_single_column_no_header() {
        let csv = b"21B91A0501\n21B91A0502\n21B91A0503\n";
        let rolls = parse_roll_numbers_from_file(csv).unwrap();
        assert_eq!(rolls, vec!["21B91A0501", "21B91A0502", "21B91A0503"]);
    }

    #[test]
    fn test_parse_roll_numbers_multi_column_no_header_digit_fallback() {
        // No recognizable header row and more than one column: falls back to
        // whichever column's first sample cell contains a digit.
        let csv = b"Alice,21B91A0501\nBob,21B91A0502\n";
        let rolls = parse_roll_numbers_from_file(csv).unwrap();
        assert_eq!(rolls, vec!["21B91A0501", "21B91A0502"]);
    }

    #[test]
    fn test_parse_roll_numbers_empty_file() {
        let rolls = parse_roll_numbers_from_file(b"").unwrap();
        assert!(rolls.is_empty());
    }

    #[test]
    fn test_parse_roll_numbers_recognizes_registration_enrollment_and_prn_headers() {
        for header in [
            "Registration Number",
            "Enrollment No",
            "Enrolment Number",
            "PRN",
            "USN",
            "Admission No.",
            "GR Number",
            "Hall Ticket No",
        ] {
            let csv = format!("{header},Name\n21B91A0501,Alice\n");
            let rolls = parse_roll_numbers_from_file(csv.as_bytes()).unwrap();
            assert_eq!(rolls, vec!["21B91A0501"], "header {header:?} should be recognized");
        }
    }

    /// A genuine "Roll Number" column must win over a leading "S.No" index
    /// column, even though "S.No" comes first left-to-right — S.No/ID/etc.
    /// are ambiguous fallback aliases, only used when no unambiguous column
    /// exists at all (see the two-pass scan in `parse_roll_numbers_from_file`).
    #[test]
    fn test_parse_roll_numbers_prefers_unambiguous_alias_over_leading_sno_column() {
        let csv = b"S.No,Roll Number,Name\n1,21B91A0501,Alice\n2,21B91A0502,Bob\n";
        let rolls = parse_roll_numbers_from_file(csv).unwrap();
        assert_eq!(rolls, vec!["21B91A0501", "21B91A0502"]);
    }

    /// With no unambiguous column at all, a weak alias like "S.No" is still
    /// better than nothing.
    #[test]
    fn test_parse_roll_numbers_falls_back_to_weak_alias_when_nothing_else_matches() {
        let csv = b"S.No,Name\n21B91A0501,Alice\n21B91A0502,Bob\n";
        let rolls = parse_roll_numbers_from_file(csv).unwrap();
        assert_eq!(rolls, vec!["21B91A0501", "21B91A0502"]);
    }
}
