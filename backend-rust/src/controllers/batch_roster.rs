// Editing the roster of an existing batch: upload a new spreadsheet to
// merge into / replace it, or add/edit/remove individual students (the
// admin UI's spreadsheet-style editor).
//
// Upload flow mirrors `admin::excel_sessions`: a dry-run parse
// (`parse_batch_students`, no DB access) -> a preview that diffs the parsed
// rows against the live roster (`preview_batch_students`, read-only) -> a
// confirm that re-computes the same diff inside a transaction and applies it
// (`sync_batch_students`). The server never trusts a client-side diff.
//
// Past attendance is keyed by roll-number *text* (`attendances.roll_number`,
// stored upper-cased, has no FK to `students`), so removing a student never
// rewrites or deletes history: their check-ins remain in the records and
// exports. Correcting a student's roll number is different — it is the same
// person, and leaving old check-ins under the old text would show them
// absent (and their check-ins as off-roster) for every earlier session — so
// `update_batch_student` carries the change through to this batch's sessions'
// attendance (and the student's passkey) in the same transaction. A file
// sync can't do that: it identifies students *by* roll number, so a changed
// roll number there is indistinguishable from "one student removed, another
// added".

use axum::{
    extract::{Json, Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    controllers::batch::{audit, find_live_batch, parse_excel},
    error::{AppError, Result},
    middleware::{validators::is_valid_email, AuthenticatedAdmin},
    models::{Student, StudentInput},
};

/// Upper bound on a roster edit, so one request can't hold the batch row
/// lock for an unbounded number of statements.
const MAX_ROSTER_STUDENTS: usize = 5000;
const MAX_FIELD_LEN: usize = 200;

/// Case-insensitive identity of a student within a batch — the same
/// `upper(roll_number)` comparison the rest of the system uses.
fn roll_key(roll_number: &str) -> String {
    roll_number.trim().to_uppercase()
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Trims, upper-cases the roll number and validates one student row. `row` is 1-based, for messages.
fn normalize_student(input: &StudentInput, row: Option<usize>) -> Result<StudentInput> {
    // Row prefix only makes sense for a multi-row roster.
    let at = row.map(|r| format!("Row {r}: ")).unwrap_or_default();
    let name = input.name.trim().to_string();
    // Roll numbers are stored trimmed and upper-cased everywhere (attendance
    // rows, exports and every comparison already use that form).
    let roll_number = input.roll_number.trim().to_uppercase();
    if name.is_empty() {
        return Err(AppError::BadRequest(format!("{at}name is required")));
    }
    if roll_number.is_empty() {
        return Err(AppError::BadRequest(format!("{at}roll number is required")));
    }
    let email = clean_optional(&input.email);
    let college_name = clean_optional(&input.college_name);
    for (label, value) in [
        ("name", Some(&name)),
        ("roll number", Some(&roll_number)),
        ("email", email.as_ref()),
        ("college", college_name.as_ref()),
    ] {
        if value.is_some_and(|v| v.chars().count() > MAX_FIELD_LEN) {
            return Err(AppError::BadRequest(format!(
                "{at}{label} is longer than {MAX_FIELD_LEN} characters"
            )));
        }
    }
    if let Some(e) = &email {
        if !is_valid_email(e) {
            return Err(AppError::BadRequest(format!(
                "{at}'{e}' is not a valid email address"
            )));
        }
    }
    Ok(StudentInput {
        name,
        roll_number,
        college_name,
        email,
    })
}

/// Normalizes a whole submitted roster and rejects duplicate roll numbers
/// (case-insensitive) within it.
fn normalize_roster(students: &[StudentInput]) -> Result<Vec<StudentInput>> {
    if students.len() > MAX_ROSTER_STUDENTS {
        return Err(AppError::BadRequest(format!(
            "A roster can have at most {MAX_ROSTER_STUDENTS} students"
        )));
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::with_capacity(students.len());
    for (i, s) in students.iter().enumerate() {
        let row = i + 1;
        let n = normalize_student(s, Some(row))?;
        if let Some(first) = seen.insert(roll_key(&n.roll_number), row) {
            return Err(AppError::BadRequest(format!(
                "Row {row}: duplicate roll number '{}' (same as row {first}; roll numbers are case-insensitive)",
                n.roll_number
            )));
        }
        out.push(n);
    }
    Ok(out)
}

/// Drops duplicate roll numbers (case/whitespace-insensitive) from a freshly
/// parsed upload — the first occurrence wins — and reports each dropped row
/// in `errors`. Shared by the roster-edit dry run and the initial batch
/// upload so both agree on what counts as a duplicate.
pub(crate) fn dedupe_parsed_students(
    parsed: Vec<StudentInput>,
    errors: &mut Vec<String>,
) -> Vec<StudentInput> {
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut students = Vec::with_capacity(parsed.len());
    for s in parsed {
        let key = roll_key(&s.roll_number);
        if let Some(first_name) = seen.get(&key) {
            errors.push(format!(
                "Duplicate roll number '{}' ('{}') ignored; kept the first occurrence ('{}')",
                s.roll_number, s.name, first_name
            ));
            continue;
        }
        seen.insert(key, s.name.clone());
        students.push(s);
    }
    students
}

/// Trims/validates each parsed row with the same rules as the roster editor
/// (`normalize_student`), skipping (and reporting) rows that fail instead of
/// failing the whole upload. Returns the clean rows.
pub(crate) fn normalize_parsed_students(
    parsed: Vec<StudentInput>,
    errors: &mut Vec<String>,
) -> Vec<StudentInput> {
    let mut out = Vec::with_capacity(parsed.len());
    for (i, s) in parsed.iter().enumerate() {
        match normalize_student(s, None) {
            Ok(n) => out.push(n),
            Err(AppError::BadRequest(reason)) => errors.push(format!(
                "Student {} ('{}') skipped: {reason}",
                i + 1,
                s.roll_number
            )),
            Err(e) => errors.push(format!(
                "Student {} ('{}') skipped: {e}",
                i + 1,
                s.roll_number
            )),
        }
    }
    out
}

/// Upper bound on a roster, shared with the initial batch upload.
pub(crate) fn ensure_roster_size(count: usize) -> Result<()> {
    if count > MAX_ROSTER_STUDENTS {
        return Err(AppError::BadRequest(format!(
            "A roster can have at most {MAX_ROSTER_STUDENTS} students"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    /// Add students that aren't in the batch yet and update the ones that
    /// are; nobody is removed.
    Merge,
    /// Make the batch's roster exactly the submitted list: add, update, and
    /// remove everyone who isn't in it.
    Replace,
}

impl SyncMode {
    fn as_str(self) -> &'static str {
        match self {
            SyncMode::Merge => "merge",
            SyncMode::Replace => "replace",
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudentFields {
    pub name: String,
    pub roll_number: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

impl From<&Student> for StudentFields {
    fn from(s: &Student) -> Self {
        Self {
            name: s.name.clone(),
            roll_number: s.roll_number.clone(),
            college_name: s.college_name.clone(),
            email: s.email.clone(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedStudent {
    pub id: Uuid,
    pub before: StudentFields,
    pub after: StudentFields,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemovedStudent {
    pub id: Uuid,
    #[serde(flatten)]
    pub fields: StudentFields,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterDiff {
    pub mode: SyncMode,
    pub added: Vec<StudentFields>,
    pub updated: Vec<UpdatedStudent>,
    pub removed: Vec<RemovedStudent>,
    pub unchanged: usize,
}

/// A planned change to the stored roster, in a form `apply_plan` can run.
struct Plan {
    diff: RosterDiff,
    /// `(student id, new position)` for every student that stays, so a
    /// replace upload can reorder to the file's order.
    positions: Vec<(Uuid, i32)>,
    /// New students, with the position each is inserted at.
    inserts: Vec<(StudentInput, i32)>,
}

/// Pure diff of `incoming` (already normalized, duplicate-free) against the
/// live roster. Students are matched by case-insensitive roll number.
///
/// An incoming optional field that is `None` (e.g. the upload had no email
/// column) leaves the stored value alone rather than blanking it; a matched
/// student keeps its stored roll-number casing.
fn plan_sync(existing: &[Student], incoming: &[StudentInput], mode: SyncMode) -> Plan {
    let by_key: HashMap<String, &Student> = existing
        .iter()
        .map(|s| (roll_key(&s.roll_number), s))
        .collect();

    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut unchanged = 0usize;
    let mut positions = Vec::new();
    let mut inserts = Vec::new();
    let mut matched: HashSet<Uuid> = HashSet::new();

    let next_position = existing
        .iter()
        .map(|s| s.position)
        .max()
        .map_or(0, |p| p + 1);
    let mut appended = 0;

    for (idx, inc) in incoming.iter().enumerate() {
        match by_key.get(&roll_key(&inc.roll_number)) {
            Some(cur) => {
                matched.insert(cur.id);
                let before = StudentFields::from(*cur);
                let after = StudentFields {
                    name: inc.name.clone(),
                    roll_number: cur.roll_number.clone(),
                    college_name: inc
                        .college_name
                        .clone()
                        .or_else(|| cur.college_name.clone()),
                    email: inc.email.clone().or_else(|| cur.email.clone()),
                };
                if before == after {
                    unchanged += 1;
                } else {
                    updated.push(UpdatedStudent {
                        id: cur.id,
                        before,
                        after,
                    });
                }
                if mode == SyncMode::Replace {
                    positions.push((cur.id, idx as i32));
                }
            }
            None => {
                added.push(StudentFields {
                    name: inc.name.clone(),
                    roll_number: inc.roll_number.clone(),
                    college_name: inc.college_name.clone(),
                    email: inc.email.clone(),
                });
                let position = match mode {
                    SyncMode::Replace => idx as i32,
                    SyncMode::Merge => {
                        appended += 1;
                        next_position + appended - 1
                    }
                };
                inserts.push((inc.clone(), position));
            }
        }
    }

    let removed = if mode == SyncMode::Replace {
        existing
            .iter()
            .filter(|s| !matched.contains(&s.id))
            .map(|s| RemovedStudent {
                id: s.id,
                fields: StudentFields::from(s),
            })
            .collect()
    } else {
        Vec::new()
    };

    Plan {
        diff: RosterDiff {
            mode,
            added,
            updated,
            removed,
            unchanged,
        },
        positions,
        inserts,
    }
}

async fn fetch_roster(db: impl sqlx::PgExecutor<'_>, batch_id: Uuid) -> Result<Vec<Student>> {
    Ok(
        sqlx::query_as::<_, Student>(
            "SELECT * FROM students WHERE batch_id = $1 ORDER BY position",
        )
        .bind(batch_id)
        .fetch_all(db)
        .await?,
    )
}

fn parse_batch_id(id: &str) -> Result<Uuid> {
    Uuid::parse_str(id).map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))
}

// =================== Dry-run parse of an uploaded file ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseStudentsResponse {
    pub students: Vec<StudentInput>,
    pub errors: Vec<String>,
}

/// Parses an uploaded spreadsheet/CSV with the same pipeline the batch
/// import uses (header-alias detection incl. `ROLL_NUMBER_ALIASES`, header-less
/// fallback), without touching the database. Duplicate roll numbers within
/// the file are dropped (first occurrence wins) and reported in `errors`.
pub async fn parse_batch_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    find_live_batch(&state.db, &auth, batch_id).await?;

    let mut file_data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        if field.name() == Some("file") {
            file_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
                    .to_vec(),
            );
        }
    }
    let file_data =
        file_data.ok_or_else(|| AppError::BadRequest("No file uploaded".to_string()))?;

    let (parsed, mut errors) = parse_excel(&file_data)?;
    let students = dedupe_parsed_students(parsed, &mut errors);
    if students.is_empty() {
        return Err(AppError::BadRequest(
            "No students with both a name and a roll number were found in this file".to_string(),
        ));
    }
    if students.len() > MAX_ROSTER_STUDENTS {
        return Err(AppError::BadRequest(format!(
            "A roster can have at most {MAX_ROSTER_STUDENTS} students"
        )));
    }

    Ok(Json(ParseStudentsResponse { students, errors }))
}

// =================== Preview / apply a roster sync ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub mode: SyncMode,
    pub students: Vec<StudentInput>,
    /// `"excel"` | `"manual"` — recorded in the audit log only.
    #[serde(default)]
    pub source: Option<String>,
}

fn validate_sync(req: &SyncRequest) -> Result<Vec<StudentInput>> {
    let incoming = normalize_roster(&req.students)?;
    if req.mode == SyncMode::Replace && incoming.is_empty() {
        return Err(AppError::BadRequest(
            "Replacing a roster needs at least one student".to_string(),
        ));
    }
    Ok(incoming)
}

/// Read-only: the added/updated/removed/unchanged breakdown a sync would
/// produce right now. Nothing is written.
pub async fn preview_batch_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<SyncRequest>,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    find_live_batch(&state.db, &auth, batch_id).await?;
    let incoming = validate_sync(&payload)?;
    let existing = fetch_roster(&state.db, batch_id).await?;
    Ok(Json(plan_sync(&existing, &incoming, payload.mode).diff))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    pub message: String,
    #[serde(flatten)]
    pub diff: RosterDiff,
    pub student_count: usize,
}

pub async fn sync_batch_students(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<SyncRequest>,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    let incoming = validate_sync(&payload)?;

    let mut tx = state.db.begin().await?;
    // Same row lock `delete_batch` takes, so a roster sync and a delete (or
    // two syncs) can't interleave.
    sqlx::query_as::<_, crate::models::Batch>(
        "SELECT * FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3) FOR UPDATE",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    let existing = fetch_roster(&mut *tx, batch_id).await?;
    let plan = plan_sync(&existing, &incoming, payload.mode);

    for u in &plan.diff.updated {
        sqlx::query(
            "UPDATE students SET name = $2, college_name = $3, email = $4 WHERE id = $1 AND batch_id = $5",
        )
        .bind(u.id)
        .bind(&u.after.name)
        .bind(&u.after.college_name)
        .bind(&u.after.email)
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
    }
    if !plan.diff.removed.is_empty() {
        let ids: Vec<Uuid> = plan.diff.removed.iter().map(|r| r.id).collect();
        sqlx::query("DELETE FROM students WHERE batch_id = $1 AND id = ANY($2)")
            .bind(batch_id)
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
    }
    for (student_id, position) in &plan.positions {
        sqlx::query("UPDATE students SET position = $2 WHERE id = $1 AND batch_id = $3")
            .bind(student_id)
            .bind(position)
            .bind(batch_id)
            .execute(&mut *tx)
            .await?;
    }
    for (s, position) in &plan.inserts {
        sqlx::query(
            "INSERT INTO students (id, batch_id, position, name, roll_number, college_name, email) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(batch_id)
        .bind(position)
        .bind(&s.name)
        .bind(&s.roll_number)
        .bind(&s.college_name)
        .bind(&s.email)
        .execute(&mut *tx)
        .await?;
    }

    let student_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE batch_id = $1")
            .bind(batch_id)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;

    let source = match payload.source.as_deref() {
        Some("excel") => "excel",
        _ => "manual",
    };
    audit(
        &state,
        &auth,
        "batch_roster_synced",
        serde_json::json!({
            "batch_id": batch_id,
            "mode": payload.mode.as_str(),
            "source": source,
            "added": plan.diff.added.len(),
            "updated": plan.diff.updated.len(),
            "removed": plan.diff.removed.len(),
            "unchanged": plan.diff.unchanged,
            "removed_roll_numbers": plan.diff.removed.iter().map(|r| r.fields.roll_number.clone()).collect::<Vec<_>>(),
        }),
    )
    .await;

    Ok(Json(SyncResponse {
        message: "Roster updated".to_string(),
        diff: plan.diff,
        student_count: student_count as usize,
    }))
}

// =================== Single-row editing (spreadsheet editor) ===================

/// Roll numbers other students in the batch already use (case-insensitive),
/// excluding `except` (the row being edited).
async fn roll_taken(
    db: impl sqlx::PgExecutor<'_>,
    batch_id: Uuid,
    roll_number: &str,
    except: Option<Uuid>,
) -> Result<bool> {
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students \
         WHERE batch_id = $1 AND upper(roll_number) = upper($2) AND ($3::uuid IS NULL OR id <> $3))",
    )
    .bind(batch_id)
    .bind(roll_number)
    .bind(except)
    .fetch_one(db)
    .await?;
    Ok(taken)
}

fn duplicate_roll_error(roll_number: &str) -> AppError {
    AppError::Conflict(format!(
        "A student with roll number '{roll_number}' already exists in this batch (roll numbers are case-insensitive)"
    ))
}

/// Locks the (live, in-scope) batch row for the rest of `tx`.
async fn lock_batch(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    auth: &AuthenticatedAdmin,
    batch_id: Uuid,
) -> Result<()> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM batches WHERE id = $1 AND deleted_at IS NULL AND ($2 = 'super_admin' OR created_by = $3) FOR UPDATE",
    )
    .bind(batch_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;
    Ok(())
}

pub async fn add_batch_student(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<StudentInput>,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    let input = normalize_student(&payload, None)?;

    let mut tx = state.db.begin().await?;
    lock_batch(&mut tx, &auth, batch_id).await?;
    if roll_taken(&mut *tx, batch_id, &input.roll_number, None).await? {
        return Err(duplicate_roll_error(&input.roll_number));
    }
    let student = sqlx::query_as::<_, Student>(
        "INSERT INTO students (id, batch_id, position, name, roll_number, college_name, email) \
         VALUES ($1, $2, (SELECT COALESCE(MAX(position) + 1, 0) FROM students WHERE batch_id = $2), $3, $4, $5, $6) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(batch_id)
    .bind(&input.name)
    .bind(&input.roll_number)
    .bind(&input.college_name)
    .bind(&input.email)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    audit(
        &state,
        &auth,
        "batch_student_added",
        serde_json::json!({ "batch_id": batch_id, "student_id": student.id, "roll_number": student.roll_number }),
    )
    .await;

    Ok((StatusCode::CREATED, Json(student)))
}

#[derive(Debug, Default)]
struct RollCascade {
    attendances: u64,
    passkey: bool,
}

/// Carries a roll-number correction (`old` -> `new`) through to the records
/// keyed by roll text, inside the caller's transaction:
///
/// * `attendances` of this batch's sessions (matched case-insensitively,
///   stored upper-cased like every check-in). A 409 if some session already
///   has a row for *both* rolls — the per-session unique key
///   (session, roll_number) would otherwise be violated, and merging two
///   check-ins automatically would silently drop one.
/// * the student's enrolled passkey (`webauthn_credentials.student_id`),
///   unless another live roster still lists the old roll (then the passkey
///   still belongs to that student there). A 409 if the new roll already has
///   its own passkey.
async fn cascade_roll_change(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_id: Uuid,
    old_roll: &str,
    new_roll: &str,
) -> Result<RollCascade> {
    let new_upper = new_roll.trim().to_uppercase();

    let colliding_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions s \
         WHERE s.batch_id = $1 \
           AND EXISTS (SELECT 1 FROM attendances a WHERE a.session_id = s.id AND upper(a.roll_number) = upper($2)) \
           AND EXISTS (SELECT 1 FROM attendances a WHERE a.session_id = s.id AND upper(a.roll_number) = $3)",
    )
    .bind(batch_id)
    .bind(old_roll)
    .bind(&new_upper)
    .fetch_one(&mut **tx)
    .await?;
    if colliding_sessions > 0 {
        return Err(AppError::Conflict(format!(
            "Can't change the roll number to '{}': in {colliding_sessions} session(s) of this batch there are already check-ins under both '{}' and '{}'. Remove or merge one of them first.",
            new_roll.trim(),
            old_roll,
            new_roll.trim()
        )));
    }

    let attendances = sqlx::query(
        "UPDATE attendances SET roll_number = $3 \
         WHERE upper(roll_number) = upper($2) \
           AND session_id IN (SELECT id FROM sessions WHERE batch_id = $1)",
    )
    .bind(batch_id)
    .bind(old_roll)
    .bind(&new_upper)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    let old_on_other_roster: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students st \
         JOIN batches b ON b.id = st.batch_id AND b.deleted_at IS NULL \
         WHERE st.batch_id <> $1 AND upper(st.roll_number) = upper($2))",
    )
    .bind(batch_id)
    .bind(old_roll)
    .fetch_one(&mut **tx)
    .await?;
    let mut passkey = false;
    if !old_on_other_roster {
        let new_has_passkey: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM webauthn_credentials WHERE upper(student_id) = $1)",
        )
        .bind(&new_upper)
        .fetch_one(&mut **tx)
        .await?;
        let old_has_passkey: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM webauthn_credentials WHERE upper(student_id) = upper($1))",
        )
        .bind(old_roll)
        .fetch_one(&mut **tx)
        .await?;
        if old_has_passkey && new_has_passkey {
            return Err(AppError::Conflict(format!(
                "Can't change the roll number to '{}': both '{}' and '{}' already have an enrolled passkey. Reset one of them first.",
                new_roll.trim(),
                old_roll,
                new_roll.trim()
            )));
        }
        if old_has_passkey {
            sqlx::query(
                "UPDATE webauthn_credentials SET student_id = $2 WHERE upper(student_id) = upper($1)",
            )
            .bind(old_roll)
            .bind(&new_upper)
            .execute(&mut **tx)
            .await?;
            passkey = true;
        }
    }

    Ok(RollCascade {
        attendances,
        passkey,
    })
}

pub async fn update_batch_student(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, student_id)): Path<(String, String)>,
    Json(payload): Json<StudentInput>,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    let student_id = Uuid::parse_str(&student_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid student ID: {}", e)))?;
    let input = normalize_student(&payload, None)?;

    let mut tx = state.db.begin().await?;
    lock_batch(&mut tx, &auth, batch_id).await?;
    let before =
        sqlx::query_as::<_, Student>("SELECT * FROM students WHERE id = $1 AND batch_id = $2")
            .bind(student_id)
            .bind(batch_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("Student not found".to_string()))?;

    if roll_taken(&mut *tx, batch_id, &input.roll_number, Some(student_id)).await? {
        return Err(duplicate_roll_error(&input.roll_number));
    }
    let cascaded = if roll_key(&before.roll_number) != roll_key(&input.roll_number) {
        cascade_roll_change(&mut tx, batch_id, &before.roll_number, &input.roll_number).await?
    } else {
        RollCascade::default()
    };
    // Unlike a file merge, an explicit edit sets exactly what was submitted
    // (an emptied email/college field clears it).
    let student = sqlx::query_as::<_, Student>(
        "UPDATE students SET name = $3, roll_number = $4, college_name = $5, email = $6 \
         WHERE id = $1 AND batch_id = $2 RETURNING *",
    )
    .bind(student_id)
    .bind(batch_id)
    .bind(&input.name)
    .bind(&input.roll_number)
    .bind(&input.college_name)
    .bind(&input.email)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    audit(
        &state,
        &auth,
        "batch_student_updated",
        serde_json::json!({
            "batch_id": batch_id,
            "student_id": student_id,
            "before": StudentFields::from(&before),
            "after": StudentFields::from(&student),
            "attendances_moved": cascaded.attendances,
            "passkey_moved": cascaded.passkey,
        }),
    )
    .await;

    Ok(Json(student))
}

pub async fn delete_batch_student(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, student_id)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let batch_id = parse_batch_id(&id)?;
    let student_id = Uuid::parse_str(&student_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid student ID: {}", e)))?;

    let mut tx = state.db.begin().await?;
    lock_batch(&mut tx, &auth, batch_id).await?;
    let removed = sqlx::query_as::<_, Student>(
        "DELETE FROM students WHERE id = $1 AND batch_id = $2 RETURNING *",
    )
    .bind(student_id)
    .bind(batch_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Student not found".to_string()))?;
    tx.commit().await?;

    audit(
        &state,
        &auth,
        "batch_student_removed",
        serde_json::json!({
            "batch_id": batch_id,
            "student_id": student_id,
            "roll_number": removed.roll_number,
            "name": removed.name,
        }),
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Student removed" })))
}

#[cfg(test)]
mod roster_tests {
    use super::*;

    fn student(roll: &str, name: &str, pos: i32) -> Student {
        Student {
            id: Uuid::new_v4(),
            batch_id: Uuid::nil(),
            position: pos,
            name: name.to_string(),
            roll_number: roll.to_string(),
            college_name: None,
            email: None,
        }
    }

    fn input(roll: &str, name: &str) -> StudentInput {
        StudentInput {
            name: name.to_string(),
            roll_number: roll.to_string(),
            college_name: None,
            email: None,
        }
    }

    #[test]
    fn merge_adds_updates_and_never_removes() {
        let existing = vec![student("R001", "Alice", 0), student("r002", "Bob", 1)];
        let incoming = vec![
            input("r001", "Alice"),
            input("R002", "Robert"),
            input("R003", "Cara"),
        ];
        let plan = plan_sync(&existing, &incoming, SyncMode::Merge);
        assert_eq!(plan.diff.unchanged, 1);
        assert_eq!(plan.diff.updated.len(), 1);
        assert_eq!(plan.diff.updated[0].after.name, "Robert");
        // Stored roll-number casing is preserved on a case-insensitive match.
        assert_eq!(plan.diff.updated[0].after.roll_number, "r002");
        assert_eq!(plan.diff.added.len(), 1);
        assert_eq!(plan.diff.added[0].roll_number, "R003");
        assert!(plan.diff.removed.is_empty());
        // New students are appended after the current last position.
        assert_eq!(plan.inserts[0].1, 2);
    }

    #[test]
    fn replace_removes_missing_and_reorders() {
        let existing = vec![student("R001", "Alice", 0), student("R002", "Bob", 1)];
        let incoming = vec![input("R003", "Cara"), input("R001", "Alice")];
        let plan = plan_sync(&existing, &incoming, SyncMode::Replace);
        assert_eq!(plan.diff.removed.len(), 1);
        assert_eq!(plan.diff.removed[0].fields.roll_number, "R002");
        assert_eq!(plan.diff.added.len(), 1);
        assert_eq!(plan.diff.unchanged, 1);
        assert_eq!(plan.inserts[0].1, 0);
        assert_eq!(plan.positions, vec![(existing[0].id, 1)]);
    }

    #[test]
    fn missing_optional_fields_keep_stored_values() {
        let mut cur = student("R001", "Alice", 0);
        cur.email = Some("a@x.test".to_string());
        cur.college_name = Some("X".to_string());
        let plan = plan_sync(&[cur], &[input("R001", "Alice")], SyncMode::Replace);
        assert_eq!(plan.diff.unchanged, 1);
        assert!(plan.diff.updated.is_empty());
    }

    #[test]
    fn normalize_roster_rejects_case_insensitive_duplicates() {
        let err = normalize_roster(&[input("ab1", "A"), input(" AB1 ", "B")]).unwrap_err();
        assert!(err.to_string().contains("duplicate roll number"), "{err}");
    }

    #[test]
    fn normalize_student_uppercases_roll_numbers() {
        let n = normalize_student(&input("  zzr001 ", "A"), None).unwrap();
        assert_eq!(n.roll_number, "ZZR001");
        let rows = normalize_roster(&[input("ab1", "A"), input("ab2", "B")]).unwrap();
        assert_eq!(rows[0].roll_number, "AB1");
    }

    #[test]
    fn normalize_student_validates_required_fields_and_email() {
        assert!(normalize_student(&input(" ", "A"), Some(1)).is_err());
        assert!(normalize_student(&input("R1", " "), Some(1)).is_err());
        let mut bad = input("R1", "A");
        bad.email = Some("not-an-email".to_string());
        assert!(normalize_student(&bad, Some(1)).is_err());
        let mut ok = input(" R1 ", " A ");
        ok.email = Some("  ".to_string());
        let n = normalize_student(&ok, Some(1)).unwrap();
        assert_eq!((n.roll_number.as_str(), n.name.as_str()), ("R1", "A"));
        assert!(n.email.is_none());
    }
}
