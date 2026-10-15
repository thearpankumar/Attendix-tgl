use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::ROLE_SUPER_ADMIN,
    controllers::validate_active_mentor_ids,
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{RecurringSessionRule, ShortLink},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecurringRuleRequest {
    /// Required unless `sessionKind` is `"intern_monitoring"` — validated in
    /// the handler, not by the type, since which is true depends on another
    /// field (the same shape as `CreateSessionRequest`'s exam-vs-normal split).
    pub location_id: Option<String>,
    pub batch_id: Option<String>,
    pub description: Option<String>,
    pub duration_minutes: i32,
    /// "HH:MM" 24-hour local times, e.g. ["09:00", "13:30"] — one or more.
    pub run_times_local: Vec<String>,
    /// IANA timezone name, e.g. "Asia/Kolkata".
    pub timezone: String,
    /// 0=Sunday..6=Saturday — at least one required for a recurring rule;
    /// ignored (and may be omitted) for a one-time schedule.
    #[serde(default)]
    pub days_of_week: Vec<i16>,
    /// "YYYY-MM-DD" — first local day (in `timezone`) a recurring rule may
    /// fire on. Omitted means today. Must not be in the past. A run time
    /// already gone by on that first day is skipped (no backfill), so the
    /// first occurrence is the next matching slot. Not allowed together
    /// with `runOnceDate`.
    pub start_date: Option<String>,
    /// "YYYY-MM-DD" — makes this a one-time schedule: the rule creates one
    /// session on this local date (in `timezone`) at the single entry of
    /// `runTimesLocal`, then deactivates itself. Must be in the future.
    pub run_once_date: Option<String>,
    pub shortlink_mode: Option<String>,
    pub custom_short_code: Option<String>,
    pub existing_short_code: Option<String>,
    /// When `shortlink_mode` is `"existing"` and the chosen code is locked to
    /// another active recurring rule, the request 400s unless this is true —
    /// in which case the other rule's lock is cleared (same effect as
    /// pausing it) and the code is reused here instead.
    #[serde(default)]
    pub force_reassign: bool,
    #[serde(default)]
    pub monitoring_enabled: bool,
    pub class_duration_minutes: Option<i32>,
    /// `"attendance"` (default) or `"intern_monitoring"`.
    pub session_kind: Option<String>,
    /// `"normal"` (default) or `"exam"` -- the mentor-session shape. An exam
    /// rule requires a batch, at least one mentor and a college name; its
    /// location is optional. Generated occurrences start at fire time.
    pub session_type: Option<String>,
    /// Mentors assigned to every generated occurrence. Mandatory for an exam
    /// rule, optional for a normal one.
    #[serde(default)]
    pub assigned_admin_ids: Vec<String>,
    pub college_name: Option<String>,
}

const SESSION_KIND_ATTENDANCE: &str = "attendance";
const SESSION_KIND_INTERN_MONITORING: &str = "intern_monitoring";
const SESSION_TYPE_NORMAL: &str = "normal";
const SESSION_TYPE_EXAM: &str = "exam";

fn validate_session_type(t: &str) -> Result<()> {
    if t != SESSION_TYPE_NORMAL && t != SESSION_TYPE_EXAM {
        return Err(AppError::BadRequest(format!("Unknown sessionType '{}'", t)));
    }
    Ok(())
}

/// Parses an optional location ID, treating empty as absent.
fn parse_optional_uuid(raw: Option<&str>, what: &str) -> Result<Option<Uuid>> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("Invalid {}: {}", what, e)))
}

fn validate_session_kind(kind: &str) -> Result<()> {
    if kind != SESSION_KIND_ATTENDANCE && kind != SESSION_KIND_INTERN_MONITORING {
        return Err(AppError::BadRequest(format!(
            "Unknown sessionKind '{}'",
            kind
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRecurringRuleRequest {
    pub location_id: Option<String>,
    pub batch_id: Option<String>,
    pub description: Option<String>,
    pub duration_minutes: Option<i32>,
    pub run_times_local: Option<Vec<String>>,
    pub timezone: Option<String>,
    pub days_of_week: Option<Vec<i16>>,
    pub monitoring_enabled: Option<bool>,
    pub class_duration_minutes: Option<i32>,
    pub assigned_admin_ids: Option<Vec<String>>,
    pub college_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRuleResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub location_id: Option<String>,
    pub batch_id: Option<String>,
    pub description: Option<String>,
    pub duration_minutes: i32,
    pub run_times_local: Vec<String>,
    pub timezone: String,
    pub days_of_week: Vec<i16>,
    pub is_active: bool,
    pub locked_short_code: Option<String>,
    pub next_run_at: DateTime<Utc>,
    pub last_generated_at: Option<DateTime<Utc>>,
    pub last_generated_session_id: Option<String>,
    pub consecutive_failures: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub monitoring_enabled: bool,
    pub class_duration_minutes: Option<i32>,
    pub session_kind: String,
    pub session_type: String,
    pub assigned_admin_ids: Vec<String>,
    pub college_name: Option<String>,
    pub start_date: Option<String>,
    pub run_once_date: Option<String>,
    /// True for a one-time schedule that has already fired.
    pub completed: bool,
}

impl From<RecurringSessionRule> for RecurringRuleResponse {
    fn from(r: RecurringSessionRule) -> Self {
        let completed = r.is_one_time() && r.last_generated_at.is_some();
        Self {
            id: r.id.to_string(),
            location_id: r.location_id.map(|id| id.to_string()),
            batch_id: r.batch_id.map(|b| b.to_string()),
            description: r.description,
            duration_minutes: r.duration_minutes,
            run_times_local: r
                .run_times_local
                .iter()
                .map(|t| t.format("%H:%M").to_string())
                .collect(),
            timezone: r.timezone,
            days_of_week: r.days_of_week,
            is_active: r.is_active,
            locked_short_code: r.locked_short_code,
            next_run_at: r.next_run_at,
            last_generated_at: r.last_generated_at,
            last_generated_session_id: r.last_generated_session_id.map(|s| s.to_string()),
            consecutive_failures: r.consecutive_failures,
            last_error: r.last_error,
            created_at: r.created_at,
            monitoring_enabled: r.monitoring_enabled,
            class_duration_minutes: r.class_duration_minutes,
            session_kind: r.session_kind,
            session_type: r.session_type,
            assigned_admin_ids: r
                .assigned_admin_ids
                .iter()
                .map(|id| id.to_string())
                .collect(),
            college_name: r.college_name,
            start_date: r.start_date.map(|d| d.to_string()),
            run_once_date: r.run_once_date.map(|d| d.to_string()),
            completed,
        }
    }
}

fn parse_run_times(raw: &[String]) -> Result<Vec<NaiveTime>> {
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "At least one time of day is required".to_string(),
        ));
    }
    raw.iter()
        .map(|s| {
            NaiveTime::parse_from_str(s.trim(), "%H:%M")
                .map_err(|_| AppError::BadRequest(format!("Invalid time '{}': expected HH:MM", s)))
        })
        .collect()
}

fn parse_date(raw: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("Invalid {}: expected YYYY-MM-DD", field)))
}

/// Parses the optional `startDate` / `runOnceDate` pair and checks them
/// against "today" in the rule's own timezone. Returns `(start_date,
/// run_once_date)`. A start date of today is allowed; one in the past, or
/// more than a year out, is rejected.
fn parse_schedule_dates(
    start_date: Option<&str>,
    run_once_date: Option<&str>,
    timezone: &str,
) -> Result<(Option<NaiveDate>, Option<NaiveDate>)> {
    let tz: chrono_tz::Tz = timezone
        .parse()
        .map_err(|_| AppError::BadRequest(format!("Unknown timezone: {}", timezone)))?;
    let today = Utc::now().with_timezone(&tz).date_naive();
    let max = today + chrono::Duration::days(366);

    let start_date = start_date
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_date(s, "startDate"))
        .transpose()?;
    let run_once_date = run_once_date
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_date(s, "runOnceDate"))
        .transpose()?;

    if start_date.is_some() && run_once_date.is_some() {
        return Err(AppError::BadRequest(
            "startDate cannot be combined with runOnceDate".to_string(),
        ));
    }
    if let Some(d) = start_date {
        if d < today {
            return Err(AppError::BadRequest(
                "startDate cannot be in the past".to_string(),
            ));
        }
        if d > max {
            return Err(AppError::BadRequest(
                "startDate must be within the next year".to_string(),
            ));
        }
    }
    if let Some(d) = run_once_date {
        if d < today {
            return Err(AppError::BadRequest(
                "The scheduled date cannot be in the past".to_string(),
            ));
        }
        if d > max {
            return Err(AppError::BadRequest(
                "The scheduled date must be within the next year".to_string(),
            ));
        }
    }
    Ok((start_date, run_once_date))
}

fn validate_days_of_week(days: &[i16]) -> Result<()> {
    if days.is_empty() {
        return Err(AppError::BadRequest(
            "At least one day of week is required".to_string(),
        ));
    }
    if days.iter().any(|d| !(0..=6).contains(d)) {
        return Err(AppError::BadRequest(
            "days_of_week entries must be 0 (Sunday) through 6 (Saturday)".to_string(),
        ));
    }
    Ok(())
}

fn validate_timezone(tz: &str) -> Result<()> {
    tz.parse::<chrono_tz::Tz>()
        .map(|_| ())
        .map_err(|_| AppError::BadRequest(format!("Unknown timezone: {}", tz)))
}

fn validate_duration(minutes: i32) -> Result<()> {
    if !(5..=480).contains(&minutes) {
        return Err(AppError::BadRequest(
            "Duration must be between 5 and 480 minutes".to_string(),
        ));
    }
    Ok(())
}

pub async fn create_recurring_rule(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateRecurringRuleRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let session_kind = payload
        .session_kind
        .clone()
        .unwrap_or_else(|| SESSION_KIND_ATTENDANCE.to_string());
    validate_session_kind(&session_kind)?;
    let is_intern_monitoring = session_kind == SESSION_KIND_INTERN_MONITORING;

    let session_type = payload
        .session_type
        .clone()
        .unwrap_or_else(|| SESSION_TYPE_NORMAL.to_string());
    validate_session_type(&session_type)?;
    let is_exam = session_type == SESSION_TYPE_EXAM;
    if is_exam && is_intern_monitoring {
        return Err(AppError::BadRequest(
            "A rule cannot be both an exam rule and an intern-monitoring rule".to_string(),
        ));
    }

    // A normal attendance rule still requires a real location (enforced
    // again at the DB level by the 0008/0014 CHECK); an intern-monitoring
    // rule has none, and an exam (mentor) rule's location is optional.
    let location_id = if is_intern_monitoring {
        None
    } else if is_exam {
        parse_optional_uuid(payload.location_id.as_deref(), "location ID")?
    } else {
        Some(
            Uuid::parse_str(payload.location_id.as_deref().unwrap_or_default())
                .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?,
        )
    };
    if let Some(id) = location_id {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM locations WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("Location not found".to_string()))?;
    }
    let batch_id = payload
        .batch_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    // Mentors: mandatory for an exam rule, optional for a normal one (so
    // they can fix a student's check-in), never for intern monitoring.
    let assigned_admin_ids = if is_intern_monitoring {
        Vec::new()
    } else {
        validate_active_mentor_ids(&state.db, &payload.assigned_admin_ids).await?
    };
    let college_name = payload
        .college_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if is_exam {
        if batch_id.is_none() {
            return Err(AppError::BadRequest(
                "A batch is required for an exam rule".to_string(),
            ));
        }
        if assigned_admin_ids.is_empty() {
            return Err(AppError::BadRequest(
                "At least one mentor is required for an exam rule".to_string(),
            ));
        }
        if college_name.is_none() {
            return Err(AppError::BadRequest(
                "College name is required for an exam rule".to_string(),
            ));
        }
    }
    let college_name = if is_exam { college_name } else { None };

    validate_duration(payload.duration_minutes)?;
    let run_times_local = parse_run_times(&payload.run_times_local)?;
    validate_timezone(&payload.timezone)?;
    let (start_date, run_once_date) = parse_schedule_dates(
        payload.start_date.as_deref(),
        payload.run_once_date.as_deref(),
        &payload.timezone,
    )?;
    // A one-time schedule fires on exactly one date at exactly one time, so
    // `days_of_week` is derived from that date rather than taken from the
    // client (the column still needs >= 1 entry).
    let days_of_week: Vec<i16> = match run_once_date {
        Some(date) => {
            if run_times_local.len() != 1 {
                return Err(AppError::BadRequest(
                    "A one-time schedule takes exactly one time of day".to_string(),
                ));
            }
            vec![date.weekday().num_days_from_sunday() as i16]
        }
        None => {
            validate_days_of_week(&payload.days_of_week)?;
            payload.days_of_week.clone()
        }
    };
    // Intern-monitoring rules always have monitoring on — that's the entire
    // point of the kind — and therefore always need a class duration, which
    // becomes the tracked work-hours span for each generated occurrence.
    if is_intern_monitoring && !payload.monitoring_enabled {
        return Err(AppError::BadRequest(
            "Intern monitoring rules must have monitoring enabled".to_string(),
        ));
    }
    if payload.monitoring_enabled {
        let minutes = payload.class_duration_minutes.ok_or_else(|| {
            AppError::BadRequest(
                "Full class duration is required when monitoring is enabled".to_string(),
            )
        })?;
        validate_duration(minutes)?;
    }

    // Intern-monitoring rules need a real locked short link too — see the
    // matching comment in `controllers::session::create_session`. The admin
    // can otherwise pick any mode same as a normal rule; only fall back to
    // "auto" when the request left it unset or explicitly asked for none,
    // since an intern rule must never end up linkless.
    let requested_mode = payload.shortlink_mode.as_deref().unwrap_or("none");
    let mode = if is_intern_monitoring && requested_mode == "none" {
        "auto"
    } else {
        requested_mode
    };
    let locked_short_code = match mode {
        "none" => None,
        "custom" => {
            let code = payload
                .custom_short_code
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if code.len() < 3
                || code.len() > 20
                || !code.chars().all(|c| c.is_alphanumeric() || c == '-')
            {
                return Err(AppError::BadRequest(
                    "Custom short code must be 3-20 alphanumeric characters or hyphens".to_string(),
                ));
            }
            let existing: Option<ShortLink> =
                sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                    .bind(&code)
                    .fetch_optional(&state.db)
                    .await?;
            if existing.is_some() {
                return Err(AppError::BadRequest(format!(
                    "Short link '/s/{}' is already taken. Please choose another custom code.",
                    code
                )));
            }
            // Reserve the code now as an unattached row — the scheduler
            // re-points its session_id to each occurrence as it's generated.
            sqlx::query(
                "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, click_count, created_at) \
                 VALUES ($1, $2, NULL, $3, true, 0, now())",
            )
            .bind(Uuid::new_v4())
            .bind(&code)
            .bind(auth.id)
            .execute(&state.db)
            .await?;
            Some(code)
        }
        "auto" => {
            let mut attempts = 0;
            let code = loop {
                let candidate = ShortLink::generate_short_code(10);
                let existing: Option<ShortLink> =
                    sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                        .bind(&candidate)
                        .fetch_optional(&state.db)
                        .await?;
                if existing.is_none() {
                    break candidate;
                }
                attempts += 1;
                if attempts > 10 {
                    return Err(AppError::Internal(
                        "Failed to generate unique short code".to_string(),
                    ));
                }
            };
            // Same unattached-reservation as "custom" — the scheduler
            // re-points session_id to each occurrence as it's generated.
            sqlx::query(
                "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, click_count, created_at) \
                 VALUES ($1, $2, NULL, $3, true, 0, now())",
            )
            .bind(Uuid::new_v4())
            .bind(&code)
            .bind(auth.id)
            .execute(&state.db)
            .await?;
            Some(code)
        }
        "existing" => {
            let code = payload
                .existing_short_code
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            if code.is_empty() {
                return Err(AppError::BadRequest(
                    "Please select an existing short link".to_string(),
                ));
            }
            let existing: Option<ShortLink> =
                sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                    .bind(&code)
                    .fetch_optional(&state.db)
                    .await?;
            if existing.is_none() {
                return Err(AppError::NotFound(format!(
                    "Existing short link '/s/{}' not found",
                    code
                )));
            }
            let locked_to_active_rule: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM recurring_session_rules WHERE locked_short_code = $1 AND is_active = true",
            )
            .bind(&code)
            .fetch_optional(&state.db)
            .await?;
            if let Some(rule_id) = locked_to_active_rule {
                if !payload.force_reassign {
                    return Err(AppError::BadRequest(format!(
                        "Short link '/s/{}' is already locked to another active recurring rule. Confirm reassignment to take it.",
                        code
                    )));
                }
                sqlx::query(
                    "UPDATE recurring_session_rules SET locked_short_code = NULL, updated_at = now() WHERE id = $1",
                )
                .bind(rule_id)
                .execute(&state.db)
                .await?;
            }
            Some(code)
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown shortlink mode '{}'",
                other
            )))
        }
    };

    let next_run_at = RecurringSessionRule::compute_next_run_at_bounded(
        &days_of_week,
        &run_times_local,
        &payload.timezone,
        Utc::now(),
        start_date,
        run_once_date,
    )
    .map_err(AppError::BadRequest)?;

    let rule: RecurringSessionRule = sqlx::query_as(
        "INSERT INTO recurring_session_rules \
         (id, location_id, batch_id, description, duration_minutes, run_times_local, timezone, days_of_week, \
          is_active, created_by, created_at, updated_at, locked_short_code, next_run_at, \
          monitoring_enabled, class_duration_minutes, session_kind, start_date, run_once_date, \
          session_type, assigned_admin_ids, college_name) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, now(), now(), $10, $11, $12, $13, $14, $15, $16, $17, $18, $19) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(location_id)
    .bind(batch_id)
    .bind(&payload.description)
    .bind(payload.duration_minutes)
    .bind(&run_times_local)
    .bind(&payload.timezone)
    .bind(&days_of_week)
    .bind(auth.id)
    .bind(&locked_short_code)
    .bind(next_run_at)
    .bind(payload.monitoring_enabled)
    .bind(payload.class_duration_minutes)
    .bind(&session_kind)
    .bind(start_date)
    .bind(run_once_date)
    .bind(&session_type)
    .bind(&assigned_admin_ids)
    .bind(&college_name)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(RecurringRuleResponse::from(rule))))
}

pub async fn get_recurring_rules(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let rules: Vec<RecurringSessionRule> = sqlx::query_as(
        "SELECT * FROM recurring_session_rules \
         WHERE ($1 = 'super_admin' OR created_by = $2) \
         ORDER BY created_at DESC",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    let response: Vec<RecurringRuleResponse> =
        rules.into_iter().map(RecurringRuleResponse::from).collect();
    Ok(Json(response))
}

async fn find_rule_for_admin(
    db: &sqlx::PgPool,
    rule_id: Uuid,
    auth: &AuthenticatedAdmin,
) -> Result<RecurringSessionRule> {
    sqlx::query_as(
        "SELECT * FROM recurring_session_rules WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(rule_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("Recurring rule not found".to_string()))
}

pub async fn update_recurring_rule(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateRecurringRuleRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;
    let rule_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid rule ID: {}", e)))?;

    let existing = find_rule_for_admin(&state.db, rule_id, &auth).await?;
    if existing.is_one_time() && existing.last_generated_at.is_some() {
        return Err(AppError::BadRequest(
            "This one-time schedule has already run and can't be edited".to_string(),
        ));
    }

    // `session_kind` is immutable after creation (not accepted by this
    // request type at all), so an intern-monitoring rule's location stays
    // None across edits the same way it started.
    let location_id = match &payload.location_id {
        Some(s) if !s.is_empty() => Some(
            Uuid::parse_str(s)
                .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?,
        ),
        Some(_) => None,
        None => existing.location_id,
    };
    if existing.session_kind == SESSION_KIND_ATTENDANCE
        && existing.session_type != SESSION_TYPE_EXAM
        && location_id.is_none()
    {
        return Err(AppError::BadRequest(
            "location_id is required for an attendance rule".to_string(),
        ));
    }
    let batch_id = match &payload.batch_id {
        Some(s) if !s.is_empty() => Some(
            Uuid::parse_str(s)
                .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?,
        ),
        Some(_) => None,
        None => existing.batch_id,
    };
    let duration_minutes = payload
        .duration_minutes
        .unwrap_or(existing.duration_minutes);
    validate_duration(duration_minutes)?;

    let run_times_local = match &payload.run_times_local {
        Some(raw) => parse_run_times(raw)?,
        None => existing.run_times_local.clone(),
    };
    let timezone = payload
        .timezone
        .clone()
        .unwrap_or(existing.timezone.clone());
    validate_timezone(&timezone)?;
    let days_of_week = payload
        .days_of_week
        .clone()
        .unwrap_or(existing.days_of_week.clone());
    validate_days_of_week(&days_of_week)?;

    // Any schedule-affecting field recomputes next_run_at from now — an
    // edit should never retroactively "owe" an occurrence that would have
    // fired under the old schedule.
    if existing.is_one_time() && run_times_local.len() != 1 {
        return Err(AppError::BadRequest(
            "A one-time schedule takes exactly one time of day".to_string(),
        ));
    }
    let next_run_at = RecurringSessionRule::compute_next_run_at_bounded(
        &days_of_week,
        &run_times_local,
        &timezone,
        Utc::now(),
        existing.start_date,
        existing.run_once_date,
    )
    .map_err(AppError::BadRequest)?;

    let description = payload.description.clone().or(existing.description.clone());

    let monitoring_enabled = payload
        .monitoring_enabled
        .unwrap_or(existing.monitoring_enabled);
    let class_duration_minutes = payload
        .class_duration_minutes
        .or(existing.class_duration_minutes);
    if monitoring_enabled {
        let minutes = class_duration_minutes.ok_or_else(|| {
            AppError::BadRequest(
                "Full class duration is required when monitoring is enabled".to_string(),
            )
        })?;
        validate_duration(minutes)?;
    }

    let assigned_admin_ids = match &payload.assigned_admin_ids {
        Some(raw) => validate_active_mentor_ids(&state.db, raw).await?,
        None => existing.assigned_admin_ids.clone(),
    };
    let college_name = match &payload.college_name {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(_) => None,
        None => existing.college_name.clone(),
    };
    if existing.session_type == SESSION_TYPE_EXAM {
        if assigned_admin_ids.is_empty() {
            return Err(AppError::BadRequest(
                "At least one mentor is required for an exam rule".to_string(),
            ));
        }
        if college_name.is_none() {
            return Err(AppError::BadRequest(
                "College name is required for an exam rule".to_string(),
            ));
        }
        if batch_id.is_none() {
            return Err(AppError::BadRequest(
                "A batch is required for an exam rule".to_string(),
            ));
        }
    }

    let updated: RecurringSessionRule = sqlx::query_as(
        "UPDATE recurring_session_rules \
         SET location_id = $1, batch_id = $2, description = $3, duration_minutes = $4, \
             run_times_local = $5, timezone = $6, days_of_week = $7, next_run_at = $8, updated_at = now(), \
             monitoring_enabled = $9, class_duration_minutes = $10, \
             assigned_admin_ids = $12, college_name = $13 \
         WHERE id = $11 \
         RETURNING *",
    )
    .bind(location_id)
    .bind(batch_id)
    .bind(&description)
    .bind(duration_minutes)
    .bind(&run_times_local)
    .bind(&timezone)
    .bind(&days_of_week)
    .bind(next_run_at)
    .bind(monitoring_enabled)
    .bind(class_duration_minutes)
    .bind(rule_id)
    .bind(&assigned_admin_ids)
    .bind(&college_name)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(RecurringRuleResponse::from(updated)))
}

/// Shared by pause/resume: flips `is_active`, and per the confirmed product
/// decision, immediately releases the locked short code on pause (not just
/// delete) — an admin might want to hand that link to a different ad-hoc
/// session the moment a rule is switched off, not just after deleting it.
/// Resuming clears any past-due `next_run_at` by recomputing it from "now",
/// so a rule paused for a week doesn't fire a backlog the moment it's back on.
async fn set_rule_active(
    state: &crate::AppState,
    auth: &AuthenticatedAdmin,
    rule_id: Uuid,
    active: bool,
) -> Result<RecurringSessionRule> {
    auth.require_role(ROLE_SUPER_ADMIN)?;
    let existing = find_rule_for_admin(&state.db, rule_id, auth).await?;

    if active {
        if existing.is_one_time() && existing.last_generated_at.is_some() {
            return Err(AppError::BadRequest(
                "This one-time schedule has already run and can't be resumed".to_string(),
            ));
        }
        let next_run_at = RecurringSessionRule::compute_next_run_at_bounded(
            &existing.days_of_week,
            &existing.run_times_local,
            &existing.timezone,
            Utc::now(),
            existing.start_date,
            existing.run_once_date,
        )
        .map_err(AppError::BadRequest)?;

        let updated: RecurringSessionRule = sqlx::query_as(
            "UPDATE recurring_session_rules \
             SET is_active = true, next_run_at = $1, consecutive_failures = 0, last_error = NULL, updated_at = now() \
             WHERE id = $2 RETURNING *",
        )
        .bind(next_run_at)
        .bind(rule_id)
        .fetch_one(&state.db)
        .await?;
        Ok(updated)
    } else {
        let updated: RecurringSessionRule = sqlx::query_as(
            "UPDATE recurring_session_rules \
             SET is_active = false, locked_short_code = NULL, updated_at = now() \
             WHERE id = $1 RETURNING *",
        )
        .bind(rule_id)
        .fetch_one(&state.db)
        .await?;
        Ok(updated)
    }
}

pub async fn pause_recurring_rule(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let rule_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid rule ID: {}", e)))?;
    let updated = set_rule_active(&state, &auth, rule_id, false).await?;
    Ok(Json(RecurringRuleResponse::from(updated)))
}

pub async fn resume_recurring_rule(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let rule_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid rule ID: {}", e)))?;
    let updated = set_rule_active(&state, &auth, rule_id, true).await?;
    Ok(Json(RecurringRuleResponse::from(updated)))
}

pub async fn delete_recurring_rule(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;
    let rule_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid rule ID: {}", e)))?;

    // Releases the locked code (if any) implicitly — nothing else references
    // it once this row is gone. Sessions this rule already generated keep
    // their own history intact via ON DELETE SET NULL on recurring_rule_id.
    let result = sqlx::query(
        "DELETE FROM recurring_session_rules WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(rule_id)
    .bind(&auth.role)
    .bind(auth.id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Recurring rule not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "message": "Recurring rule deleted successfully"
    })))
}
