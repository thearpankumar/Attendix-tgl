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
}

pub async fn get_batches(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<GetBatchesQuery>,
) -> Result<impl IntoResponse> {
    let batches = sqlx::query_as::<_, Batch>(
        "SELECT * FROM batches WHERE created_by = $1 ORDER BY created_at DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    let counts: Vec<(Uuid, i64)> =
        sqlx::query_as("SELECT batch_id, COUNT(*) FROM students GROUP BY batch_id")
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
         WHERE eb.created_by = $1",
    )
    .bind(auth.id)
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

    let batch =
        sqlx::query_as::<_, Batch>("SELECT * FROM batches WHERE id = $1 AND created_by = $2")
            .bind(batch_id)
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

pub async fn delete_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let batch_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    let result = sqlx::query("DELETE FROM batches WHERE id = $1 AND created_by = $2")
        .bind(batch_id)
        .bind(auth.id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Batch not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "message": "Batch deleted successfully"
    })))
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

    let batch_name =
        batch_name.unwrap_or_else(|| format!("Batch_{}", Utc::now().format("%Y%m%d_%H%M%S")));

    let (students, errors) = parse_excel(&file_data)?;

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
            students_skipped: 0,
            errors,
        }),
    ))
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
            for row in range.rows() {
                let cells: Vec<String> = row.iter().filter_map(format_cell).collect();
                if !cells.is_empty() {
                    raw_rows.push(cells);
                }
            }
        }
    } else if let Ok(mut wb) = open_workbook_from_rs::<Xls<_>, _>(cursor.clone()) {
        if let Some(Ok(range)) = wb.worksheet_range_at(0) {
            for row in range.rows() {
                let cells: Vec<String> = row.iter().filter_map(format_cell).collect();
                if !cells.is_empty() {
                    raw_rows.push(cells);
                }
            }
        }
    } else if let Ok(mut wb) = open_workbook_from_rs::<Ods<_>, _>(cursor.clone()) {
        if let Some(Ok(range)) = wb.worksheet_range_at(0) {
            for row in range.rows() {
                let cells: Vec<String> = row.iter().filter_map(format_cell).collect();
                if !cells.is_empty() {
                    raw_rows.push(cells);
                }
            }
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

fn parse_excel(data: &[u8]) -> Result<(Vec<StudentInput>, Vec<String>)> {
    let raw_rows = read_raw_rows(data)?;

    let mut students = Vec::new();
    let mut errors = Vec::new();

    if raw_rows.is_empty() {
        return Ok((students, errors));
    }

    let roll_aliases = [
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
        "id",
        "studentid",
        "studentno",
        "stdid",
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
        "slno",
        "sno",
        "srno",
        "serialno",
    ];

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

    // 1. Header row matching
    let header_row = &raw_rows[0];
    for (i, cell_str) in header_row.iter().enumerate() {
        let norm = normalize_header(cell_str);
        if name_col.is_none() && name_aliases.contains(&norm.as_str()) {
            name_col = Some(i);
        } else if roll_col.is_none() && roll_aliases.contains(&norm.as_str()) {
            roll_col = Some(i);
        } else if email_col.is_none() && email_aliases.contains(&norm.as_str()) {
            email_col = Some(i);
        } else if college_col.is_none() && college_aliases.contains(&norm.as_str()) {
            college_col = Some(i);
        }
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
    fn test_parse_csv_fallback_without_headers() {
        let csv_data = b"21B91A0505,David Miller\n21B91A0506,Eve Wilson\n";
        let (students, errors) = parse_excel(csv_data).unwrap();
        assert!(errors.is_empty());
        assert_eq!(students.len(), 2);
        assert_eq!(students[0].roll_number, "21B91A0505");
        assert_eq!(students[0].name, "David Miller");
    }
}
