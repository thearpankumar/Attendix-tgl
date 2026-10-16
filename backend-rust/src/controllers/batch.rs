use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use calamine::{open_workbook_from_rs, Ods, Reader, Xls, Xlsx};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::BATCHES_LIST_MAX_PAGE_SIZE,
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Batch, BatchCreate, Student, StudentInput},
};

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "studentCount")]
    pub student_count: usize,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// The Batches list view is a tagged union of manually-created batches and
/// Excel-upload-generated ones — see migration 0003. `kind` distinguishes
/// them so the frontend can tag/deep-link session-batches instead of
/// treating them like a regular selectable batch.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedBatchResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub student_count: i64,
    pub created_at: chrono::DateTime<Utc>,
    /// "manual" | "session"
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub linked_session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchCreateResponse {
    pub message: String,
    pub batch: BatchResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDetailResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub students: Vec<Student>,
    #[serde(rename = "studentCount")]
    pub student_count: usize,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

/// Inserts `students` for `batch_id`, in order, inside `tx`.
async fn insert_students(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_id: Uuid,
    students: &[StudentInput],
) -> Result<()> {
    for (position, student) in students.iter().enumerate() {
        sqlx::query(
            "INSERT INTO students (id, batch_id, position, name, roll_number, college_name, email) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(batch_id)
        .bind(position as i32)
        .bind(&student.name)
        .bind(&student.roll_number)
        .bind(&student.college_name)
        .bind(&student.email)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn fetch_students(pool: &sqlx::PgPool, batch_id: Uuid) -> Result<Vec<Student>> {
    let students = sqlx::query_as::<_, Student>(
        "SELECT * FROM students WHERE batch_id = $1 ORDER BY position",
    )
    .bind(batch_id)
    .fetch_all(pool)
    .await?;
    Ok(students)
}

pub async fn create_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<BatchCreate>,
) -> Result<impl IntoResponse> {
    let mut tx = state.db.begin().await?;

    let batch = sqlx::query_as::<_, Batch>(
        "INSERT INTO batches (id, name, description, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(auth.id)
    .bind(Utc::now())
    .fetch_one(&mut *tx)
    .await?;

    insert_students(&mut tx, batch.id, &payload.students).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(BatchCreateResponse {
            message: "Batch created successfully".to_string(),
            batch: BatchResponse {
                id: batch.id.to_string(),
                name: batch.name,
                description: batch.description,
                student_count: payload.students.len(),
                created_at: batch.created_at.to_rfc3339(),
            },
        }),
    ))
}

/// `limit`/`offset` are both optional and both-or-nothing in practice: the
/// Batches page's infinite scroll passes both to fetch one page at a time;
/// every other caller (e.g. the Sessions page's batch-picker dropdowns,
/// which need the complete list) omits them and gets everything, exactly as
/// before pagination existed — this keeps the endpoint's default behavior
/// unchanged for callers that were never updated to paginate.
#[derive(Debug, Deserialize)]
pub struct GetBatchesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Case-insensitive substring match against the batch name, applied to
    /// both the manual-batch and excel-batch (session-batch) branches below
    /// before they're merged — the Batches page's search bar.
    pub search: Option<String>,
}

pub async fn get_batches(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<GetBatchesQuery>,
) -> Result<impl IntoResponse> {
    let search = query
        .search
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(crate::utils::input::escape_like);

    let batches = sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches \
         WHERE deleted_at IS NULL AND ($1 = 'super_admin' OR created_by = $2) \
           AND ($3::text IS NULL OR name ILIKE '%' || $3 || '%' ESCAPE '\\') \
         ORDER BY created_at DESC",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .bind(&search)
    .fetch_all(&state.db)
    .await?;

    // Scoped to just the batch ids already filtered above (role + search),
    // not a system-wide scan of every student for every admin's page load.
    let batch_ids: Vec<Uuid> = batches.iter().map(|b| b.id).collect();
    let counts: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT batch_id, COUNT(*) FROM students WHERE batch_id = ANY($1) GROUP BY batch_id")
            .bind(&batch_ids)
            .fetch_all(&state.db)
            .await?;
    let count_for = |batch_id: Uuid| -> i64 {
        counts
            .iter()
            .find(|(id, _)| *id == batch_id)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    };

    let mut response: Vec<UnifiedBatchResponse> = batches
        .into_iter()
        .map(|batch| UnifiedBatchResponse {
            id: batch.id.to_string(),
            student_count: count_for(batch.id),
            name: batch.name,
            description: batch.description,
            created_at: batch.created_at,
            kind: "manual",
            linked_session_id: None,
        })
        .collect();

    // Excel-generated batches are always created alongside exactly one
    // session (see admin::excel_sessions::commit), so the join can never
    // find a dangling excel_batches row — no separate existence check needed
    // the way the manual-batch path handles a possibly-unset session.batch_id.
    #[derive(sqlx::FromRow)]
    struct ExcelBatchRow {
        id: Uuid,
        name: String,
        description: Option<String>,
        created_at: chrono::DateTime<Utc>,
        student_count: i64,
        session_id: Uuid,
    }
    let excel_rows: Vec<ExcelBatchRow> = sqlx::query_as(
        "SELECT eb.id, eb.name, eb.description, eb.created_at, s.id AS session_id, \
                (SELECT COUNT(*) FROM excel_batch_students WHERE excel_batch_id = eb.id) AS student_count \
         FROM excel_batches eb \
         JOIN sessions s ON s.excel_batch_id = eb.id \
         WHERE ($1 = 'super_admin' OR eb.created_by = $2) \
           AND ($3::text IS NULL OR eb.name ILIKE '%' || $3 || '%' ESCAPE '\\')",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .bind(&search)
    .fetch_all(&state.db)
    .await?;

    response.extend(excel_rows.into_iter().map(|r| UnifiedBatchResponse {
        id: r.id.to_string(),
        name: r.name,
        description: r.description,
        student_count: r.student_count,
        created_at: r.created_at,
        kind: "session",
        linked_session_id: Some(r.session_id.to_string()),
    }));

    response.sort_by_key(|b| std::cmp::Reverse(b.created_at));

    // Both source queries above are still unfiltered/unlimited (they always
    // have been — the manual/excel split makes DB-level LIMIT/OFFSET
    // unreliable without a combined SQL UNION, since global order can only
    // be determined after both sides are merged) — pagination is applied
    // in-memory to the already fully-sorted combined list, purely to shrink
    // the JSON response for callers that opt in via `limit`.
    if let Some(limit) = query.limit {
        let limit = limit.clamp(1, BATCHES_LIST_MAX_PAGE_SIZE) as usize;
        let offset = query.offset.unwrap_or(0).max(0) as usize;
        response = response.into_iter().skip(offset).take(limit).collect();
    }

    Ok(Json(response))
}

pub async fn get_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let batch_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    let batch = sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    let students = fetch_students(&state.db, batch_id).await?;

    Ok(Json(BatchDetailResponse {
        id: batch.id.to_string(),
        name: batch.name,
        description: batch.description,
        student_count: students.len(),
        students,
        created_by: batch.created_by.to_string(),
        created_at: batch.created_at,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRangeQuery {
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOverviewResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub total_students: i64,
    pub total_sessions_in_range: i64,
}

/// Lightweight stats for the Batch Details page's Overview tab: student
/// count is unconditional, session count is scoped to `dateFrom`/`dateTo`
/// (both optional — omitted means unbounded/all-time) via `created_at`,
/// which unlike `starts_at` is always set (see idx_sessions_batch_created,
/// migration 0011).
pub async fn get_batch_overview(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Query(range): Query<DateRangeQuery>,
) -> Result<impl IntoResponse> {
    let batch_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    let batch = sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    let total_students: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE batch_id = $1")
        .bind(batch_id)
        .fetch_one(&state.db)
        .await?;

    let total_sessions_in_range: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions \
         WHERE batch_id = $1 \
           AND ($2::timestamptz IS NULL OR created_at >= $2) \
           AND ($3::timestamptz IS NULL OR created_at <= $3)",
    )
    .bind(batch_id)
    .bind(range.date_from)
    .bind(range.date_to)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(BatchOverviewResponse {
        id: batch.id.to_string(),
        name: batch.name,
        description: batch.description,
        created_at: batch.created_at,
        total_students,
        total_sessions_in_range,
    }))
}

/// What deleting a batch would affect — the Batches page shows this in its
/// confirmation dialog before the admin commits.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteImpactResponse {
    pub session_count: i64,
    pub active_rule_count: i64,
    pub rule_count: i64,
    /// `"archived"` when sessions/rules reference the batch (history is kept),
    /// `"deleted"` when nothing does and the batch is removed outright.
    pub mode: &'static str,
}

async fn batch_references(
    db: impl sqlx::PgExecutor<'_>,
    batch_id: Uuid,
) -> Result<(i64, i64, i64)> {
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT COUNT(*) FROM sessions WHERE batch_id = $1), \
            (SELECT COUNT(*) FROM recurring_session_rules WHERE batch_id = $1), \
            (SELECT COUNT(*) FROM recurring_session_rules WHERE batch_id = $1 AND is_active)",
    )
    .bind(batch_id)
    .fetch_one(db)
    .await?;
    Ok(counts)
}

pub async fn get_batch_delete_impact(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let batch_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;
    find_live_batch(&state.db, &auth, batch_id).await?;
    let (session_count, rule_count, active_rule_count) =
        batch_references(&state.db, batch_id).await?;
    Ok(Json(BatchDeleteImpactResponse {
        session_count,
        active_rule_count,
        rule_count,
        mode: if session_count + rule_count > 0 {
            "archived"
        } else {
            "deleted"
        },
    }))
}

/// Ownership-scoped lookup of a batch that has not been deleted/archived.
pub(crate) async fn find_live_batch(
    db: impl sqlx::PgExecutor<'_>,
    auth: &AuthenticatedAdmin,
    batch_id: Uuid,
) -> Result<Batch> {
    sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))
}

/// Rejects a batch that doesn't exist or has been deleted/archived. Used
/// wherever a *new* session or recurring rule is pointed at a batch — a
/// deleted batch must not be picked up again, though sessions that already
/// reference it keep working (see `delete_batch`).
pub(crate) async fn ensure_batch_assignable(
    db: impl sqlx::PgExecutor<'_>,
    batch_id: Uuid,
) -> Result<()> {
    let live: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM batches WHERE id = $1 AND deleted_at IS NULL")
            .bind(batch_id)
            .fetch_optional(db)
            .await?;
    live.map(|_| ())
        .ok_or_else(|| AppError::BadRequest("Batch not found or has been deleted".to_string()))
}

/// Deletes a batch.
///
/// A batch nothing references is removed outright (its students cascade). A
/// batch that sessions or recurring rules still reference is *archived*
/// instead (`deleted_at` set; row and roster kept) — see migration 0015 for
/// why: those sessions' roster, absent list, stats and exports are all
/// derived from the batch, and attendance rows are keyed by roll number text,
/// so nothing historical changes. Recurring rules pointing at the batch are
/// paused (`is_active = false`) with an explanatory `last_error`, since a rule
/// can't generate sessions for a deleted batch. A paused rule keeps its
/// `locked_short_code`; only active rules reserve a code (the unique index is
/// partial on `is_active`), so others may reuse that code while it is paused.
pub async fn delete_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let batch_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    let mut tx = state.db.begin().await?;

    // Row lock so a concurrent session/rule create can't slip a reference in
    // between the reference check and the delete.
    let batch = sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3) FOR UPDATE",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    let (session_count, rule_count, _) = batch_references(&mut *tx, batch_id).await?;

    let mode = if session_count + rule_count == 0 {
        sqlx::query("DELETE FROM batches WHERE id = $1")
            .bind(batch_id)
            .execute(&mut *tx)
            .await?;
        "deleted"
    } else {
        sqlx::query("UPDATE batches SET deleted_at = now() WHERE id = $1")
            .bind(batch_id)
            .execute(&mut *tx)
            .await?;
        let reason = format!("Batch \"{}\" was deleted", batch.name);
        sqlx::query(
            "UPDATE recurring_session_rules \
             SET is_active = false, last_error = $2, updated_at = now() \
             WHERE batch_id = $1 AND is_active",
        )
        .bind(batch_id)
        .bind(&reason)
        .execute(&mut *tx)
        .await?;
        "archived"
    };

    tx.commit().await?;

    audit(
        &state,
        &auth,
        "batch_deleted",
        serde_json::json!({
            "batch_id": batch_id,
            "name": batch.name,
            "mode": mode,
            "sessions_kept": session_count,
            "rules_paused": rule_count,
        }),
    )
    .await;

    Ok(Json(serde_json::json!({
        "message": "Batch deleted successfully",
        "mode": mode,
        "sessionsKept": session_count,
        "rulesPaused": rule_count,
    })))
}

/// Audit log entry for an admin batch action. The audit log is a detective
/// control, not a gate — a failure to record never fails the request.
pub(crate) async fn audit(
    state: &crate::AppState,
    auth: &AuthenticatedAdmin,
    event: &str,
    detail: serde_json::Value,
) {
    if let Err(e) =
        crate::models::record_audit_event(&state.db, Some(auth.id), event, detail, None).await
    {
        tracing::error!(error = %e, event, "Failed to record audit log entry");
    }
}

#[derive(Debug, Serialize)]
pub struct UploadBatchResponse {
    #[serde(rename = "batchId")]
    pub batch_id: String,
    pub name: String,
    #[serde(rename = "studentsImported")]
    pub students_imported: usize,
    #[serde(rename = "studentsSkipped")]
    pub students_skipped: usize,
    /// The caller already has a live batch with this (case-insensitive) name.
    #[serde(rename = "duplicateName")]
    pub duplicate_name: bool,
    pub errors: Vec<String>,
}

pub async fn upload_batch_excel(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut batch_name: Option<String> = None;
    let mut description: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
                        .to_vec(),
                );
            }
            "name" => {
                batch_name =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Failed to read name: {}", e))
                    })?);
            }
            "description" => {
                description = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read description: {}", e))
                })?);
            }
            _ => {}
        }
    }

    let file_data =
        file_data.ok_or_else(|| AppError::BadRequest("No file uploaded".to_string()))?;

    // An omitted name falls back to a timestamped one; a name that is present
    // but blank is a mistake, not a request for the default.
    let batch_name = match batch_name {
        Some(n) if n.trim().is_empty() => {
            return Err(AppError::BadRequest("Batch name is required".to_string()))
        }
        Some(n) => n.trim().to_string(),
        None => format!("Batch_{}", crate::utils::ist::format_filename_stamp(Utc::now())),
    };
    if batch_name.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "Batch name is longer than 200 characters".to_string(),
        ));
    }

    let (parsed, mut errors) = parse_excel(&file_data)?;
    let parsed_count = parsed.len();
    // Same dedupe/normalisation the roster editor applies: a duplicate roll
    // number (case/whitespace-insensitive) is skipped and reported, never
    // stored twice (nothing at the DB level would stop it).
    let students = crate::controllers::batch_roster::dedupe_parsed_students(parsed, &mut errors);
    let students =
        crate::controllers::batch_roster::normalize_parsed_students(students, &mut errors);
    if students.is_empty() {
        return Err(AppError::BadRequest(
            "No students with both a name and a roll number were found in this file".to_string(),
        ));
    }
    crate::controllers::batch_roster::ensure_roster_size(students.len())?;

    // Batch names aren't unique (nothing keys on them), so a repeat is allowed
    // but flagged — the admin may have meant to update the existing roster.
    let duplicate_name: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM batches WHERE deleted_at IS NULL AND created_by = $1 AND lower(name) = lower($2))",
    )
    .bind(auth.id)
    .bind(&batch_name)
    .fetch_one(&state.db)
    .await?;
    if duplicate_name {
        errors.push(format!(
            "You already have a batch named '{batch_name}'; this one was still created (batch names aren't unique)."
        ));
    }

    let mut tx = state.db.begin().await?;

    let batch = sqlx::query_as::<_, Batch>(
        "INSERT INTO batches (id, name, description, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&batch_name)
    .bind(&description)
    .bind(auth.id)
    .bind(Utc::now())
    .fetch_one(&mut *tx)
    .await?;

    insert_students(&mut tx, batch.id, &students).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(UploadBatchResponse {
            batch_id: batch.id.to_string(),
            name: batch_name,
            students_imported: students.len(),
            students_skipped: parsed_count - students.len(),
            duplicate_name,
            errors,
        }),
    ))
}

/// Converts one spreadsheet row to strings, keeping column positions: a
/// blank cell becomes an empty string rather than being dropped, otherwise
/// every cell to its right would shift one column left (an empty Email cell
/// would make the College value read as the email). Returns `None` for a row
/// with no content at all.
fn row_cells(row: &[calamine::Data]) -> Option<Vec<String>> {
    let cells: Vec<String> = row
        .iter()
        .map(|c| format_cell(c).unwrap_or_default())
        .collect();
    cells.iter().any(|c| !c.is_empty()).then_some(cells)
}

fn format_cell(cell: &calamine::Data) -> Option<String> {
    match cell {
        calamine::Data::String(s) => {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        calamine::Data::Int(n) => Some(n.to_string()),
        calamine::Data::Float(f) => {
            if f.fract() == 0.0 {
                Some((*f as i64).to_string())
            } else {
                Some(f.to_string())
            }
        }
        calamine::Data::DateTime(d) => Some(d.to_string()),
        calamine::Data::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(crate) fn normalize_header(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Opens `data` as a spreadsheet (Xlsx → Xls → Ods, in that order) or, if
/// none of those parse, falls back to treating it as raw comma-separated
/// text. Shared by `parse_excel` (batch/student roster import) and the
/// Excel-session dry-run parser (`admin::excel_sessions`), which both need
/// the same "just open whatever the admin uploaded" behavior but apply
/// different column-alias logic on top of the resulting rows.
pub(crate) fn read_raw_rows(data: &[u8]) -> Result<Vec<Vec<String>>> {
    let cursor = Cursor::new(data);
    let mut raw_rows: Vec<Vec<String>> = Vec::new();

    if let Ok(mut wb) = open_workbook_from_rs::<Xlsx<_>, _>(cursor.clone()) {
        if let Some(Ok(range)) = wb.worksheet_range_at(0) {
            raw_rows.extend(range.rows().filter_map(row_cells));
        }
    } else if let Ok(mut wb) = open_workbook_from_rs::<Xls<_>, _>(cursor.clone()) {
        if let Some(Ok(range)) = wb.worksheet_range_at(0) {
            raw_rows.extend(range.rows().filter_map(row_cells));
        }
    } else if let Ok(mut wb) = open_workbook_from_rs::<Ods<_>, _>(cursor.clone()) {
        if let Some(Ok(range)) = wb.worksheet_range_at(0) {
            raw_rows.extend(range.rows().filter_map(row_cells));
        }
    } else if let Ok(text) = std::str::from_utf8(data) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let cells: Vec<String> = line
                .split(',')
                .map(|s| s.trim_matches('"').trim().to_string())
                .collect();
            if !cells.is_empty() {
                raw_rows.push(cells);
            }
        }
    } else {
        return Err(AppError::BadRequest(
            "Failed to open file. Unsupported or corrupted spreadsheet/CSV format.".to_string(),
        ));
    }

    Ok(raw_rows)
}

/// Header aliases recognized as "this column is the roll/register number",
/// case/punctuation-insensitive via `normalize_header`. `pub(crate)` — also
/// reused by `batch_analytics::parse_roll_numbers_from_file` for the global
/// roll-number Lookup's file-upload path, so both importers agree on what
/// counts as a roll-number column without duplicating the list.
///
/// Covers the terminology actually used across Indian colleges/universities
/// (researched, not guessed): "roll number" is the common class-level term
/// (may change by year/semester); "registration"/"enrollment"/"enrolment"
/// number and PRN (Permanent Registration Number) are the more durable
/// university-level identifiers, and are often the *same* number for a
/// single-degree student even though they're conceptually distinct; USN
/// (University Seat Number, VTU/Karnataka), SRN (Student Registration
/// Number, PESU and similar), hall ticket / HT number (AP/Telangana exam
/// identifier), GR number (General Register number, common in school-style
/// registers), admission number, and seat number round out the set.
pub(crate) const ROLL_NUMBER_ALIASES: &[&str] = &[
    "roll",
    "rollno",
    "rollnumber",
    "rollnum",
    "register",
    "registerno",
    "registernumber",
    "regno",
    "regnumber",
    "regnno",
    "regnum",
    "registration",
    "registrationno",
    "registrationnumber",
    "registrationnum",
    "studentid",
    "studentno",
    "stdid",
    "studentcode",
    "enrollment",
    "enrollmentno",
    "enrollmentnumber",
    "enrolment",
    "enrolmentno",
    "enrolmentnumber",
    "usn",
    "hallticket",
    "hallticketno",
    "htno",
    "admission",
    "admissionno",
    "admissionnumber",
    "admno",
    "admnno",
    "prn",
    "srn",
    "gr",
    "grno",
    "grnumber",
    "generalregisterno",
    "generalregisternumber",
    "matricno",
    "matriculationno",
    "matriculationnumber",
    "seatno",
    "seatnumber",
];

/// Header aliases that plausibly mean "roll/register number" but are
/// generic enough to also be an unrelated column — most commonly a plain
/// row index ("S.No", "Sr. No.") that happens to sit next to (and, if
/// checked with equal priority, would shadow) the real roll-number column.
/// Kept separate from `ROLL_NUMBER_ALIASES` so callers that care about this
/// ambiguity (`batch_analytics::parse_roll_numbers_from_file`) can treat it
/// as a lower-priority fallback, tried only once no unambiguous alias
/// matched any column.
pub(crate) const ROLL_NUMBER_WEAK_ALIASES: &[&str] = &["id", "slno", "sno", "srno", "serialno"];

pub(crate) fn parse_excel(data: &[u8]) -> Result<(Vec<StudentInput>, Vec<String>)> {
    let raw_rows = read_raw_rows(data)?;

    let mut students = Vec::new();
    let mut errors = Vec::new();

    if raw_rows.is_empty() {
        return Ok((students, errors));
    }

    let name_aliases = [
        "name",
        "studentname",
        "fullname",
        "student",
        "nameofthestudent",
        "candidate",
        "candidatename",
        "stname",
    ];

    let email_aliases = ["email", "emailid", "emailaddress", "mail", "mailid"];

    let college_aliases = [
        "college",
        "collegename",
        "institution",
        "institute",
        "dept",
        "department",
        "branch",
    ];

    let mut name_col: Option<usize> = None;
    let mut roll_col: Option<usize> = None;
    let mut email_col: Option<usize> = None;
    let mut college_col: Option<usize> = None;
    // An "S.No"/"Sr. No."/"ID" column is only a roll number of last resort:
    // it must not shadow a real roll/register-number column that happens to
    // sit to its right, so it's tracked separately and used only if no
    // unambiguous alias matched anywhere in the header.
    let mut weak_roll_col: Option<usize> = None;

    // 1. Header row matching
    let header_row = &raw_rows[0];
    for (i, cell_str) in header_row.iter().enumerate() {
        let norm = normalize_header(cell_str);
        if name_col.is_none() && name_aliases.contains(&norm.as_str()) {
            name_col = Some(i);
        } else if roll_col.is_none() && ROLL_NUMBER_ALIASES.contains(&norm.as_str()) {
            roll_col = Some(i);
        } else if weak_roll_col.is_none() && ROLL_NUMBER_WEAK_ALIASES.contains(&norm.as_str()) {
            weak_roll_col = Some(i);
        } else if email_col.is_none() && email_aliases.contains(&norm.as_str()) {
            email_col = Some(i);
        } else if college_col.is_none() && college_aliases.contains(&norm.as_str()) {
            college_col = Some(i);
        }
    }

    if roll_col.is_none() {
        roll_col = weak_roll_col;
    }

    let start_row_idx = if name_col.is_some() || roll_col.is_some() {
        1
    } else {
        0
    };

    // 2. Fallback heuristic if headers were not explicitly matched:
    if name_col.is_none() || roll_col.is_none() {
        if let Some(sample_row) = raw_rows.get(start_row_idx) {
            if sample_row.len() >= 2 {
                let col0_val = &sample_row[0];
                let col1_val = &sample_row[1];

                let col0_has_digit = col0_val.chars().any(|c| c.is_ascii_digit());
                let col1_has_digit = col1_val.chars().any(|c| c.is_ascii_digit());

                if name_col.is_none() && roll_col.is_none() {
                    if col0_has_digit && !col1_has_digit {
                        roll_col = Some(0);
                        name_col = Some(1);
                    } else if !col0_has_digit && col1_has_digit {
                        name_col = Some(0);
                        roll_col = Some(1);
                    } else {
                        roll_col = Some(0);
                        name_col = Some(1);
                    }
                } else if roll_col.is_none() {
                    let taken = name_col.unwrap();
                    roll_col = (0..sample_row.len()).find(|&i| i != taken);
                } else if name_col.is_none() {
                    let taken = roll_col.unwrap();
                    name_col = (0..sample_row.len()).find(|&i| i != taken);
                }
            }
        }
    }

    for (row_offset, row) in raw_rows.iter().skip(start_row_idx).enumerate() {
        let get_val = |col: Option<usize>| -> Option<String> {
            col.and_then(|i| row.get(i).map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
        };

        let name = get_val(name_col);
        let roll_number = get_val(roll_col);
        let email = get_val(email_col);
        let college_name = get_val(college_col);

        match (&name, &roll_number) {
            (Some(n), Some(r)) if !n.trim().is_empty() && !r.trim().is_empty() => {
                students.push(StudentInput {
                    name: n.trim().to_string(),
                    roll_number: r.trim().to_string(),
                    email: email.map(|e| e.trim().to_string()),
                    college_name: college_name.map(|c| c.trim().to_string()),
                });
            }
            _ => {
                if name.is_some() || roll_number.is_some() {
                    errors.push(format!(
                        "Row {}: Missing required fields (name/roll_number)",
                        row_offset + start_row_idx + 1
                    ));
                }
            }
        }
    }

    Ok((students, errors))
}

#[cfg(test)]
mod batch_tests {
    use super::*;

    #[test]
    fn test_normalize_header() {
        assert_eq!(normalize_header("Register No."), "registerno");
        assert_eq!(
            normalize_header("Registration Number"),
            "registrationnumber"
        );
        assert_eq!(normalize_header("Roll No."), "rollno");
        assert_eq!(normalize_header("Reg. No"), "regno");
        assert_eq!(normalize_header("Student ID"), "studentid");
        assert_eq!(normalize_header("Full Name"), "fullname");
    }

    #[test]
    fn test_parse_csv_register_number_first() {
        let csv_data = b"Register Number,Student Name,Email\n21B91A0501,Alice Smith,alice@example.com\n21B91A0502,Bob Jones,bob@example.com\n";
        let (students, errors) = parse_excel(csv_data).unwrap();
        assert!(errors.is_empty());
        assert_eq!(students.len(), 2);
        assert_eq!(students[0].roll_number, "21B91A0501");
        assert_eq!(students[0].name, "Alice Smith");
        assert_eq!(students[0].email.as_deref(), Some("alice@example.com"));
        assert_eq!(students[1].roll_number, "21B91A0502");
        assert_eq!(students[1].name, "Bob Jones");
    }

    #[test]
    fn test_parse_csv_name_first_registration_second() {
        let csv_data =
            b"Full Name,Registration No.,Department\nCharlie Brown,REG2024001,Computer Science\n";
        let (students, errors) = parse_excel(csv_data).unwrap();
        assert!(errors.is_empty());
        assert_eq!(students.len(), 1);
        assert_eq!(students[0].name, "Charlie Brown");
        assert_eq!(students[0].roll_number, "REG2024001");
        assert_eq!(
            students[0].college_name.as_deref(),
            Some("Computer Science")
        );
    }

    #[test]
    fn test_parse_prefers_real_roll_column_over_serial_number_column() {
        let csv_data = b"S.No,Reg No.,Student Name\n1,R001,Alice\n2,R002,Bob\n";
        let (students, errors) = parse_excel(csv_data).unwrap();
        assert!(errors.is_empty());
        assert_eq!(students[0].roll_number, "R001");
        assert_eq!(students[1].name, "Bob");
        // ...but a lone weak alias is still accepted as the roll column.
        let (students, _) = parse_excel(b"ID,Name\nA1,Cara\n").unwrap();
        assert_eq!(students[0].roll_number, "A1");
    }

    #[test]
    fn test_row_cells_keeps_blank_cells_in_place() {
        use calamine::Data;
        let row = [
            Data::String("R001".into()),
            Data::String("Alice".into()),
            Data::Empty,
            Data::String("CollegeX".into()),
        ];
        assert_eq!(
            row_cells(&row).unwrap(),
            vec!["R001", "Alice", "", "CollegeX"]
        );
        assert!(row_cells(&[Data::Empty, Data::String("  ".into())]).is_none());
    }

    #[test]
    fn test_parse_csv_fallback_without_headers() {
        let csv_data = b"21B91A0505,David Miller\n21B91A0506,Eve Wilson\n";
        let (students, errors) = parse_excel(csv_data).unwrap();
        assert!(errors.is_empty());
        assert_eq!(students.len(), 2);
        assert_eq!(students[0].roll_number, "21B91A0505");
        assert_eq!(students[0].name, "David Miller");
    }
}
