// Excel-based bulk session creation: upload one master roster spreadsheet
// (see samplefiles/ATT MASTER-TEMPLATE-*.csv) and detect one batch+session
// per distinct (track, class, session_time) group. Two-step flow:
//   1. `parse_excel_sessions` — dry-run, no DB writes, returns the detected
//      groups as JSON for the admin to review/edit in the upload UI.
//   2. `commit_excel_sessions` — takes the (possibly edited) group data back
//      as JSON and actually creates the excel_batches/excel_batch_students/
//      sessions/session_admins rows, one transaction per included group so
//      one group's failure doesn't roll back the others.

use axum::{
    extract::{Json, Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, NaiveDate, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::ROLE_ADMIN,
    controllers::batch::{normalize_header, read_raw_rows},
    error::{AppError, Result},
    middleware::{validators::is_valid_email, AuthenticatedAdmin},
    models::{record_audit_event, Admin, ExcelStudentInput, Session},
};

// =================== Dry-run parse ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMentor {
    pub email: String,
    pub matched_admin_id: Option<String>,
    pub matched_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedStudent {
    pub name: String,
    pub roll_number: String,
    pub email: Option<String>,
    pub round: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedGroup {
    pub group_index: usize,
    pub track: String,
    pub class_label: String,
    pub session_time_raw: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i32,
    pub college_name: String,
    pub mentors: Vec<ParsedMentor>,
    pub unmatched_mentor_emails: Vec<String>,
    pub mentor_email_conflict: bool,
    pub student_count: usize,
    pub students: Vec<ParsedStudent>,
    pub issues: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseSessionsResponse {
    pub source_filename: Option<String>,
    pub session_date: String,
    pub total_rows: usize,
    pub total_groups: usize,
    pub parse_errors: Vec<String>,
    pub groups: Vec<ParsedGroup>,
}

/// Turns `"4:00 PM - 6:00 PM"` into 24h `("16:00", "18:00", 120)`.
fn parse_time_range(raw: &str) -> std::result::Result<(String, String, i32), String> {
    static TIME_RANGE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)^\s*(\d{1,2}):(\d{2})\s*(AM|PM)\s*-\s*(\d{1,2}):(\d{2})\s*(AM|PM)\s*$")
            .unwrap()
    });

    let caps = TIME_RANGE_RE
        .captures(raw.trim())
        .ok_or_else(|| format!("Could not parse time range \"{}\"", raw))?;

    let to_24h = |hour: &str, minute: &str, meridiem: &str| -> Option<(u32, u32)> {
        let mut h: u32 = hour.parse().ok()?;
        let m: u32 = minute.parse().ok()?;
        if !(1..=12).contains(&h) || m > 59 {
            return None;
        }
        let is_pm = meridiem.eq_ignore_ascii_case("pm");
        if h == 12 {
            h = 0;
        }
        if is_pm {
            h += 12;
        }
        Some((h, m))
    };

    let (start_h, start_m) = to_24h(&caps[1], &caps[2], &caps[3])
        .ok_or_else(|| format!("Invalid start time in \"{}\"", raw))?;
    let (end_h, end_m) = to_24h(&caps[4], &caps[5], &caps[6])
        .ok_or_else(|| format!("Invalid end time in \"{}\"", raw))?;

    let start_minutes = (start_h * 60 + start_m) as i32;
    let mut end_minutes = (end_h * 60 + end_m) as i32;
    if end_minutes <= start_minutes {
        // Defensive: a range that appears to cross midnight (or is
        // zero-length) isn't something this template ever produces, but
        // don't silently emit a bogus/negative duration if it happens.
        end_minutes += 24 * 60;
    }
    let duration = end_minutes - start_minutes;
    if duration <= 0 || duration > 480 {
        return Err(format!(
            "Session time range \"{}\" resolves to an implausible duration",
            raw
        ));
    }

    Ok((
        format!("{:02}:{:02}", start_h, start_m),
        format!("{:02}:{:02}", end_h, end_m),
        duration,
    ))
}

struct ColumnMap {
    name: Option<usize>,
    roll: Option<usize>,
    email: Option<usize>,
    track: Option<usize>,
    class: Option<usize>,
    session_time: Option<usize>,
    round: Option<usize>,
    assigned_mentor: Option<usize>,
}

fn find_columns(header_row: &[String]) -> ColumnMap {
    let name_aliases = ["studentname", "name", "student", "fullname"];
    let roll_aliases = [
        "registrationnumber",
        "regno",
        "registration",
        "regnumber",
        "registerno",
        "rollnumber",
        "rollno",
        "roll",
    ];
    let email_aliases = ["email", "emailid", "emailaddress"];
    let track_aliases = ["track", "course"];
    let class_aliases = ["class", "location", "room"];
    let session_time_aliases = ["sessiontime", "time", "timeslot", "sessiontimeslot"];
    let round_aliases = ["round", "elective", "language"];
    let assigned_mentor_aliases = ["assignedmentor", "assignedmentors", "mentor", "mentors"];

    let mut cols = ColumnMap {
        name: None,
        roll: None,
        email: None,
        track: None,
        class: None,
        session_time: None,
        round: None,
        assigned_mentor: None,
    };

    for (i, cell) in header_row.iter().enumerate() {
        let norm = normalize_header(cell);
        if cols.name.is_none() && name_aliases.contains(&norm.as_str()) {
            cols.name = Some(i);
        } else if cols.roll.is_none() && roll_aliases.contains(&norm.as_str()) {
            cols.roll = Some(i);
        } else if cols.email.is_none() && email_aliases.contains(&norm.as_str()) {
            cols.email = Some(i);
        } else if cols.track.is_none() && track_aliases.contains(&norm.as_str()) {
            cols.track = Some(i);
        } else if cols.class.is_none() && class_aliases.contains(&norm.as_str()) {
            cols.class = Some(i);
        } else if cols.session_time.is_none() && session_time_aliases.contains(&norm.as_str()) {
            cols.session_time = Some(i);
        } else if cols.round.is_none() && round_aliases.contains(&norm.as_str()) {
            cols.round = Some(i);
        } else if cols.assigned_mentor.is_none() && assigned_mentor_aliases.contains(&norm.as_str())
        {
            cols.assigned_mentor = Some(i);
        }
    }

    cols
}

struct GroupAccumulator {
    track: String,
    class_label: String,
    session_time_raw: String,
    students: Vec<ParsedStudent>,
    mentor_emails: Vec<String>,
    raw_mentor_values_seen: HashSet<String>,
    conflict: bool,
}

struct GroupingOutcome {
    groups: Vec<GroupAccumulator>,
    parse_errors: Vec<String>,
    total_rows: usize,
}

/// Pure grouping logic (no DB access), split out from `parse_excel_sessions`
/// so it's directly unit-testable against the sample template. Groups rows
/// by exact `(track, class, session_time)` — the confirmed grouping key —
/// and collects each group's students plus any `assigned_mentor` emails
/// found (deduped, with cross-row disagreement flagged rather than
/// rejected).
fn group_rows(raw_rows: &[Vec<String>]) -> Result<GroupingOutcome> {
    if raw_rows.is_empty() {
        return Err(AppError::BadRequest(
            "The uploaded file is empty".to_string(),
        ));
    }

    let cols = find_columns(&raw_rows[0]);
    let missing: Vec<&str> = [
        (cols.name.is_none(), "student name"),
        (cols.roll.is_none(), "registration/roll number"),
        (cols.track.is_none(), "track"),
        (cols.class.is_none(), "class"),
        (cols.session_time.is_none(), "session_time"),
    ]
    .into_iter()
    .filter_map(|(missing, label)| missing.then_some(label))
    .collect();
    if !missing.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Could not find required column(s) in the header row: {}",
            missing.join(", ")
        )));
    }

    let get_val = |row: &[String], col: Option<usize>| -> Option<String> {
        col.and_then(|i| row.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let mut groups: Vec<GroupAccumulator> = Vec::new();
    let mut group_index: HashMap<(String, String, String), usize> = HashMap::new();
    let mut parse_errors: Vec<String> = Vec::new();
    let mut total_rows = 0usize;

    for (row_offset, row) in raw_rows.iter().skip(1).enumerate() {
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let line_no = row_offset + 2;

        let name = get_val(row, cols.name);
        let roll_number = get_val(row, cols.roll);
        let track = get_val(row, cols.track);
        let class_label = get_val(row, cols.class);
        let session_time_raw = get_val(row, cols.session_time);

        let (Some(name), Some(roll_number), Some(track), Some(class_label), Some(session_time_raw)) =
            (name, roll_number, track, class_label, session_time_raw)
        else {
            parse_errors.push(format!(
                "Row {}: missing student name, registration number, track, class, or session_time",
                line_no
            ));
            continue;
        };

        total_rows += 1;
        let email = get_val(row, cols.email);
        let round = get_val(row, cols.round);

        let key = (track.clone(), class_label.clone(), session_time_raw.clone());
        let idx = *group_index.entry(key).or_insert_with(|| {
            groups.push(GroupAccumulator {
                track: track.clone(),
                class_label: class_label.clone(),
                session_time_raw: session_time_raw.clone(),
                students: Vec::new(),
                mentor_emails: Vec::new(),
                raw_mentor_values_seen: HashSet::new(),
                conflict: false,
            });
            groups.len() - 1
        });
        let group = &mut groups[idx];

        group.students.push(ParsedStudent {
            name,
            roll_number,
            email,
            round,
        });

        if let Some(raw_mentor) = get_val(row, cols.assigned_mentor) {
            let normalized = raw_mentor.to_lowercase();
            group.raw_mentor_values_seen.insert(normalized.clone());
            if group.raw_mentor_values_seen.len() > 1 {
                group.conflict = true;
            }
            for email in normalized.split(',') {
                let email = email.trim();
                if !email.is_empty()
                    && is_valid_email(email)
                    && !group.mentor_emails.contains(&email.to_string())
                {
                    group.mentor_emails.push(email.to_string());
                }
            }
        }
    }

    Ok(GroupingOutcome {
        groups,
        parse_errors,
        total_rows,
    })
}

pub async fn parse_excel_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut source_filename: Option<String> = None;
    let mut session_date_raw: Option<String> = None;
    let mut default_college_name = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                source_filename = field.file_name().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
                        .to_vec(),
                );
            }
            "sessionDate" => {
                session_date_raw = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read sessionDate: {}", e))
                })?);
            }
            "collegeName" => {
                default_college_name = field.text().await.unwrap_or_default();
            }
            _ => {}
        }
    }

    let file_data =
        file_data.ok_or_else(|| AppError::BadRequest("No file uploaded".to_string()))?;
    let session_date_str = session_date_raw
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("sessionDate is required".to_string()))?;
    let session_date = NaiveDate::parse_from_str(session_date_str.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("sessionDate must be YYYY-MM-DD".to_string()))?;

    let raw_rows = read_raw_rows(&file_data)?;
    let GroupingOutcome {
        groups,
        parse_errors,
        total_rows,
    } = group_rows(&raw_rows)?;

    // Resolve mentor emails against admins in one batch lookup.
    let all_emails: HashSet<String> = groups
        .iter()
        .flat_map(|g| g.mentor_emails.iter().cloned())
        .collect();
    let admin_by_email: HashMap<String, Admin> = if all_emails.is_empty() {
        HashMap::new()
    } else {
        let emails: Vec<String> = all_emails.into_iter().collect();
        sqlx::query_as::<_, Admin>("SELECT * FROM admins WHERE lower(email) = ANY($1)")
            .bind(&emails)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .map(|a| (a.email.to_lowercase(), a))
            .collect()
    };

    let mut parsed_groups = Vec::with_capacity(groups.len());
    for (i, g) in groups.into_iter().enumerate() {
        let mut issues = Vec::new();

        let (start_time, end_time, duration_minutes) = match parse_time_range(&g.session_time_raw) {
            Ok(v) => v,
            Err(msg) => {
                issues.push(msg);
                ("00:00".to_string(), "00:00".to_string(), 60)
            }
        };

        if g.conflict {
            issues.push(
                "assigned_mentor differs across rows in this group — showing the union of all emails found".to_string(),
            );
        }

        let mut mentors = Vec::new();
        let mut unmatched = Vec::new();
        for email in &g.mentor_emails {
            match admin_by_email.get(email) {
                Some(admin) if admin.role == ROLE_ADMIN && admin.is_active => {
                    mentors.push(ParsedMentor {
                        email: email.clone(),
                        matched_admin_id: Some(admin.id.to_string()),
                        matched_name: Some(
                            admin.full_name.clone().unwrap_or(admin.username.clone()),
                        ),
                    });
                }
                _ => unmatched.push(email.clone()),
            }
        }

        parsed_groups.push(ParsedGroup {
            group_index: i,
            track: g.track,
            class_label: g.class_label,
            session_time_raw: g.session_time_raw,
            start_time,
            end_time,
            duration_minutes,
            college_name: default_college_name.clone(),
            student_count: g.students.len(),
            students: g.students,
            mentors,
            unmatched_mentor_emails: unmatched,
            mentor_email_conflict: g.conflict,
            issues,
        });
    }

    Ok(Json(ParseSessionsResponse {
        source_filename,
        session_date: session_date.format("%Y-%m-%d").to_string(),
        total_rows,
        total_groups: parsed_groups.len(),
        parse_errors,
        groups: parsed_groups,
    }))
}

// =================== Commit ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitGroup {
    pub group_index: i64,
    pub track: String,
    pub class_label: String,
    pub session_time_raw: String,
    /// RFC3339 — combined client-side from the review UI's date + start time,
    /// exactly like the manual exam-session create form already does.
    pub starts_at: String,
    pub duration_minutes: i32,
    pub college_name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub assigned_admin_ids: Vec<String>,
    #[serde(default)]
    pub raw_mentor_emails: Vec<String>,
    pub students: Vec<ExcelStudentInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSessionsRequest {
    pub source_filename: Option<String>,
    pub session_date: String,
    pub groups: Vec<CommitGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitGroupResult {
    pub group_index: i64,
    pub success: bool,
    pub session_id: Option<String>,
    pub excel_batch_id: Option<String>,
    pub students_imported: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSessionsResponse {
    pub results: Vec<CommitGroupResult>,
    pub created_count: usize,
    pub failed_count: usize,
}

async fn commit_one_group(
    db: &sqlx::PgPool,
    auth_id: Uuid,
    session_date: NaiveDate,
    source_filename: Option<&str>,
    group: &CommitGroup,
) -> Result<(Uuid, Uuid, usize)> {
    if group.students.is_empty() {
        return Err(AppError::BadRequest(
            "No students in this group".to_string(),
        ));
    }
    if !(5..=480).contains(&group.duration_minutes) {
        return Err(AppError::BadRequest(
            "Duration must be 5-480 minutes".to_string(),
        ));
    }
    if group.college_name.trim().is_empty() {
        return Err(AppError::BadRequest("College name is required".to_string()));
    }
    let starts_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&group.starts_at)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AppError::BadRequest(format!("Invalid start time: {}", e)))?;

    let mentor_ids: Vec<Uuid> = {
        let mut ids = Vec::with_capacity(group.assigned_admin_ids.len());
        for raw_id in &group.assigned_admin_ids {
            let raw_id = raw_id.trim();
            if raw_id.is_empty() {
                continue;
            }
            let mentor_id = Uuid::parse_str(raw_id)
                .map_err(|e| AppError::BadRequest(format!("Invalid mentor ID: {}", e)))?;
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM admins WHERE id = $1 AND role = $2 AND is_active = true",
            )
            .bind(mentor_id)
            .bind(ROLE_ADMIN)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Every assigned mentor must be an active admin/mentor account".to_string(),
                )
            })?;
            ids.push(mentor_id);
        }
        if ids.is_empty() {
            return Err(AppError::BadRequest(
                "At least one mentor is required".to_string(),
            ));
        }
        ids
    };

    let mut tx = db.begin().await?;

    let name = format!(
        "{} — {} — {}",
        group.track, group.class_label, group.session_time_raw
    );
    let excel_batch_id: Uuid = sqlx::query_scalar(
        "INSERT INTO excel_batches ( \
            id, name, description, source_filename, session_date, track, class_label, \
            session_time_raw, duration_minutes, college_name, raw_mentor_emails, created_by, created_at \
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now()) \
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(&name)
    .bind(&group.description)
    .bind(source_filename)
    .bind(session_date)
    .bind(&group.track)
    .bind(&group.class_label)
    .bind(&group.session_time_raw)
    .bind(group.duration_minutes)
    .bind(group.college_name.trim())
    .bind(&group.raw_mentor_emails)
    .bind(auth_id)
    .fetch_one(&mut *tx)
    .await?;

    for (position, student) in group.students.iter().enumerate() {
        sqlx::query(
            "INSERT INTO excel_batch_students (id, excel_batch_id, position, name, roll_number, email, round) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(excel_batch_id)
        .bind(position as i32)
        .bind(&student.name)
        .bind(&student.roll_number)
        .bind(&student.email)
        .bind(&student.round)
        .execute(&mut *tx)
        .await?;
    }

    let token = Session::generate_token();
    let expires_at = Utc::now() + chrono::Duration::minutes(group.duration_minutes as i64);

    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions ( \
            id, location_id, batch_id, excel_batch_id, token_hash, token_prefix, description, \
            created_by, college_name, starts_at, is_active, expires_at, rotation_count, totp_secret, created_at \
         ) VALUES ($1, NULL, NULL, $2, $3, $4, $5, $6, $7, $8, true, $9, 0, $10, now()) \
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(excel_batch_id)
    .bind(Session::hash_token(&token))
    .bind(Session::get_token_prefix(&token))
    .bind(group.description.clone().unwrap_or_else(|| name.clone()))
    .bind(auth_id)
    .bind(group.college_name.trim())
    .bind(starts_at)
    .bind(expires_at)
    .bind(Session::generate_totp_secret())
    .fetch_one(&mut *tx)
    .await?;

    for mentor_id in &mentor_ids {
        sqlx::query("INSERT INTO session_admins (session_id, admin_id) VALUES ($1, $2)")
            .bind(session_id)
            .bind(mentor_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok((session_id, excel_batch_id, group.students.len()))
}

pub async fn commit_excel_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CommitSessionsRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;

    if payload.groups.is_empty() {
        return Err(AppError::BadRequest(
            "At least one session group is required".to_string(),
        ));
    }

    let session_date = NaiveDate::parse_from_str(payload.session_date.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("sessionDate must be YYYY-MM-DD".to_string()))?;

    let mut results = Vec::with_capacity(payload.groups.len());
    let mut created_count = 0usize;
    let mut failed_count = 0usize;

    for group in &payload.groups {
        match commit_one_group(
            &state.db,
            auth.id,
            session_date,
            payload.source_filename.as_deref(),
            group,
        )
        .await
        {
            Ok((session_id, excel_batch_id, students_imported)) => {
                created_count += 1;
                if let Err(e) = record_audit_event(
                    &state.db,
                    Some(auth.id),
                    "excel_session_created",
                    serde_json::json!({
                        "session_id": session_id,
                        "excel_batch_id": excel_batch_id,
                        "track": group.track,
                        "class_label": group.class_label,
                        "session_time_raw": group.session_time_raw,
                        "students_imported": students_imported,
                    }),
                    None,
                )
                .await
                {
                    tracing::error!(error = %e, "Failed to record audit log entry for excel_session_created");
                }
                results.push(CommitGroupResult {
                    group_index: group.group_index,
                    success: true,
                    session_id: Some(session_id.to_string()),
                    excel_batch_id: Some(excel_batch_id.to_string()),
                    students_imported: Some(students_imported),
                    error: None,
                });
            }
            Err(e) => {
                failed_count += 1;
                results.push(CommitGroupResult {
                    group_index: group.group_index,
                    success: false,
                    session_id: None,
                    excel_batch_id: None,
                    students_imported: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(CommitSessionsResponse {
            results,
            created_count,
            failed_count,
        }),
    ))
}

#[cfg(test)]
mod excel_sessions_tests {
    use super::*;

    #[test]
    fn test_parse_time_range_basic() {
        assert_eq!(
            parse_time_range("4:00 PM - 6:00 PM").unwrap(),
            ("16:00".to_string(), "18:00".to_string(), 120)
        );
        assert_eq!(
            parse_time_range("2:00 PM - 4:00 PM").unwrap(),
            ("14:00".to_string(), "16:00".to_string(), 120)
        );
        assert_eq!(
            parse_time_range("8:00 AM - 10:00 AM").unwrap(),
            ("08:00".to_string(), "10:00".to_string(), 120)
        );
        // Noon/midnight boundary handling.
        assert_eq!(
            parse_time_range("12:00 PM - 1:00 PM").unwrap(),
            ("12:00".to_string(), "13:00".to_string(), 60)
        );
        assert_eq!(
            parse_time_range("11:30 pm - 11:45 pm").unwrap(),
            ("23:30".to_string(), "23:45".to_string(), 15)
        );
    }

    #[test]
    fn test_parse_time_range_rejects_garbage() {
        assert!(parse_time_range("NA").is_err());
        assert!(parse_time_range("").is_err());
        assert!(parse_time_range("4:00 - 6:00").is_err());
    }

    #[test]
    fn test_find_columns_matches_master_template_header() {
        let header: Vec<String> = "student_name,registration_number,email,track,class,session_time,round,status,marked_by,marked_at,assigned_mentor"
            .split(',')
            .map(|s| s.to_string())
            .collect();
        let cols = find_columns(&header);
        assert_eq!(cols.name, Some(0));
        assert_eq!(cols.roll, Some(1));
        assert_eq!(cols.email, Some(2));
        assert_eq!(cols.track, Some(3));
        assert_eq!(cols.class, Some(4));
        assert_eq!(cols.session_time, Some(5));
        assert_eq!(cols.round, Some(6));
        assert_eq!(cols.assigned_mentor, Some(10));
    }

    #[test]
    fn test_group_rows_mentor_union_and_conflict() {
        let raw_rows: Vec<Vec<String>> = vec![
            vec![
                "student_name",
                "registration_number",
                "email",
                "track",
                "class",
                "session_time",
                "round",
                "status",
                "marked_by",
                "marked_at",
                "assigned_mentor",
            ],
            vec![
                "Alice",
                "R1",
                "alice@example.com",
                "DSA",
                "CLS 1",
                "2:00 PM - 4:00 PM",
                "Python",
                "",
                "",
                "",
                "mentor1@example.com",
            ],
            vec![
                "Bob",
                "R2",
                "bob@example.com",
                "DSA",
                "CLS 1",
                "2:00 PM - 4:00 PM",
                "Java",
                "",
                "",
                "",
                "mentor1@example.com, mentor2@example.com",
            ],
        ]
        .into_iter()
        .map(|row| row.into_iter().map(|s| s.to_string()).collect())
        .collect();

        let outcome = group_rows(&raw_rows).unwrap();
        assert_eq!(outcome.total_rows, 2);
        assert!(outcome.parse_errors.is_empty());
        assert_eq!(outcome.groups.len(), 1);
        let group = &outcome.groups[0];
        assert_eq!(group.students.len(), 2);
        // Union of both rows' assigned_mentor values, deduped.
        assert_eq!(group.mentor_emails.len(), 2);
        assert!(group
            .mentor_emails
            .contains(&"mentor1@example.com".to_string()));
        assert!(group
            .mentor_emails
            .contains(&"mentor2@example.com".to_string()));
        // Two distinct raw assigned_mentor cell values in the same group -> flagged.
        assert!(group.conflict);
    }

    #[test]
    fn test_group_rows_against_master_template_sample() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../samplefiles/ATT MASTER-TEMPLATE-08-06-2026 - AN.csv");
        let data = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("failed to read sample file at {:?}: {}", path, e));

        let raw_rows = crate::controllers::batch::read_raw_rows(&data).unwrap();
        let outcome = group_rows(&raw_rows).unwrap();

        // Confirmed against the sample file directly: 3174 data rows,
        // 35 distinct (track, class, session_time) groups, no parse errors.
        assert_eq!(outcome.total_rows, 3174);
        assert_eq!(outcome.groups.len(), 35);
        assert!(
            outcome.parse_errors.is_empty(),
            "unexpected parse errors: {:?}",
            outcome.parse_errors
        );

        let total_students: usize = outcome.groups.iter().map(|g| g.students.len()).sum();
        assert_eq!(total_students, 3174);
    }
}
