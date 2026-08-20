// Cross-session filtering/aggregation for the admin Sessions page: the
// "which track/class/time-slot/marking-status sessions do I actually have"
// question that a single `sessions` row can't answer on its own, since
// track/class/time-slot live on `excel_batches` and marking-progress can
// only be known by joining each session's roster (`students` or
// `excel_batch_students`) against `attendances`.
//
// `query_session_records` is the single source of truth for this, shared by
// the sessions list (`get_sessions`, filtered + capped), the global stats
// overview below, and filtered bulk export — so "what counts as a match"
// never drifts between what's shown on screen and what gets exported.

use axum::{
    extract::{Json, Query, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::Result, middleware::AuthenticatedAdmin, models::Session};

/// Shared filter criteria for session-record queries. `present`/`absent` and
/// marking-progress are always computed against a session's roster, never
/// against raw attendance rows — a student a mentor explicitly marked absent
/// and a student the mentor never touched are both "not present", but only
/// the latter is what makes a session "not started"/"partial".
#[derive(Debug, Clone, Default)]
pub struct SessionRecordFilters {
    pub location_id: Option<Uuid>,
    pub date_start: Option<DateTime<Utc>>,
    pub date_end: Option<DateTime<Utc>>,
    pub batch_ids: Option<Vec<Uuid>>,
    pub class_labels: Option<Vec<String>>,
    pub tracks: Option<Vec<String>>,
    pub time_slots: Option<Vec<String>>,
    /// `"not_started" | "partial" | "complete" | "no_roster"`
    pub marking_status: Option<String>,
    pub search: Option<String>,
}

fn parse_date_range(date: Option<&str>) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let Some(date_str) = date else {
        return (None, None);
    };
    let trimmed = date_str.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    let Ok(naive_date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") else {
        return (None, None);
    };
    let start = naive_date
        .and_hms_opt(0, 0, 0)
        .map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
    let end = naive_date
        .and_hms_opt(23, 59, 59)
        .map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
    (start, end)
}

fn split_csv(raw: Option<&str>) -> Option<Vec<String>> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let values: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn non_empty_uuids(values: Vec<Uuid>) -> Option<Vec<Uuid>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn non_empty_strings(values: Vec<String>) -> Option<Vec<String>> {
    let values: Vec<String> = values
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// GET query-string shape shared by `/sessions` and `/sessions/stats-overview`.
/// Multi-value filters are comma-separated (`tracks=DSA,ML`) since axum's
/// default query extractor doesn't parse repeated-key arrays.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecordQueryParams {
    #[serde(alias = "location_id")]
    pub location_id: Option<String>,
    pub date: Option<String>,
    pub batch_ids: Option<String>,
    pub class_labels: Option<String>,
    pub tracks: Option<String>,
    pub time_slots: Option<String>,
    pub marking_status: Option<String>,
    pub search: Option<String>,
    /// Infinite-scroll paging for `GET /sessions` — ignored by
    /// `/stats-overview`, which always scans everything regardless of what's
    /// currently loaded on screen. Omitting both preserves the original
    /// unpaginated behavior (defaults to `SESSIONS_LIST_PAGE_SIZE`/0).
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl SessionRecordQueryParams {
    pub fn into_filters(self) -> SessionRecordFilters {
        let (date_start, date_end) = parse_date_range(self.date.as_deref());
        let batch_ids = split_csv(self.batch_ids.as_deref())
            .map(|ids| ids.iter().filter_map(|s| Uuid::parse_str(s).ok()).collect())
            .and_then(non_empty_uuids);
        SessionRecordFilters {
            location_id: self
                .location_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|s| Uuid::parse_str(s).ok()),
            date_start,
            date_end,
            batch_ids,
            class_labels: split_csv(self.class_labels.as_deref()),
            tracks: split_csv(self.tracks.as_deref()),
            time_slots: split_csv(self.time_slots.as_deref()),
            marking_status: self
                .marking_status
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            search: self
                .search
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

/// POST body shape (real JSON arrays) used by `/sessions/export-filtered`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecordFilterBody {
    pub location_id: Option<String>,
    pub date: Option<String>,
    #[serde(default)]
    pub batch_ids: Vec<String>,
    #[serde(default)]
    pub class_labels: Vec<String>,
    #[serde(default)]
    pub tracks: Vec<String>,
    #[serde(default)]
    pub time_slots: Vec<String>,
    pub marking_status: Option<String>,
    pub search: Option<String>,
}

impl SessionRecordFilterBody {
    pub fn into_filters(self) -> SessionRecordFilters {
        let (date_start, date_end) = parse_date_range(self.date.as_deref());
        let batch_ids = non_empty_uuids(
            self.batch_ids
                .iter()
                .filter_map(|s| Uuid::parse_str(s.trim()).ok())
                .collect(),
        );
        SessionRecordFilters {
            location_id: self
                .location_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|s| Uuid::parse_str(s).ok()),
            date_start,
            date_end,
            batch_ids,
            class_labels: non_empty_strings(self.class_labels),
            tracks: non_empty_strings(self.tracks),
            time_slots: non_empty_strings(self.time_slots),
            marking_status: self
                .marking_status
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            search: self
                .search
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

/// One matching session's roster-progress stats, as computed by
/// `query_session_records`. `roster_size == 0` means the session has no
/// batch/excel-batch roster attached at all (a plain GPS check-in) — its
/// marking status is always `"no_roster"`, excluded from the Not
/// Started/Partial/Complete filter buckets (only matches "All").
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRecordStats {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub track: Option<String>,
    pub class_label: Option<String>,
    pub session_time_raw: Option<String>,
    pub roster_size: i64,
    pub marked_count: i64,
    pub present_count: i64,
}

impl SessionRecordStats {
    pub fn marking_status(&self) -> &'static str {
        if self.roster_size == 0 {
            "no_roster"
        } else if self.marked_count == 0 {
            "not_started"
        } else if self.marked_count < self.roster_size {
            "partial"
        } else {
            "complete"
        }
    }

    pub fn attendance_percent(&self) -> Option<f64> {
        if self.roster_size == 0 {
            None
        } else {
            Some((self.present_count as f64 / self.roster_size as f64) * 100.0)
        }
    }
}

const SESSION_RECORDS_SQL: &str = r#"
WITH scoped_sessions AS (
    SELECT s.id, s.created_at, s.batch_id, s.excel_batch_id,
           eb.track AS track,
           COALESCE(eb.class_label, l.name) AS class_label,
           eb.session_time_raw AS session_time_raw
    FROM sessions s
    LEFT JOIN excel_batches eb ON eb.id = s.excel_batch_id
    LEFT JOIN locations l ON l.id = s.location_id
    WHERE ($1 = 'super_admin'
           OR EXISTS (
                SELECT 1 FROM session_admins sa WHERE sa.session_id = s.id AND sa.admin_id = $2))
      AND ($3::uuid IS NULL OR s.location_id = $3)
      AND ($4::timestamptz IS NULL OR s.created_at >= $4)
      AND ($5::timestamptz IS NULL OR s.created_at <= $5)
      AND ($6::uuid[] IS NULL OR COALESCE(s.batch_id, s.excel_batch_id) = ANY($6))
      AND ($7::text[] IS NULL OR COALESCE(eb.class_label, l.name) = ANY($7))
      AND ($8::text[] IS NULL OR eb.track = ANY($8))
      AND ($9::text[] IS NULL OR eb.session_time_raw = ANY($9))
),
roster AS (
    SELECT ss.id AS session_id, st.roll_number, st.name
    FROM scoped_sessions ss
    JOIN students st ON st.batch_id = ss.batch_id
    WHERE ss.batch_id IS NOT NULL
    UNION ALL
    SELECT ss.id AS session_id, ebs.roll_number, ebs.name
    FROM scoped_sessions ss
    JOIN excel_batch_students ebs ON ebs.excel_batch_id = ss.excel_batch_id
    WHERE ss.excel_batch_id IS NOT NULL
),
roster_marks AS (
    SELECT r.session_id, r.roll_number, r.name, a.status AS marked_status
    FROM roster r
    LEFT JOIN attendances a
      ON a.session_id = r.session_id AND upper(a.roll_number) = upper(r.roll_number)
),
roster_agg AS (
    SELECT session_id,
           COUNT(*) AS roster_size,
           COUNT(*) FILTER (WHERE marked_status IS NOT NULL) AS marked_count,
           COUNT(*) FILTER (WHERE marked_status = 'present') AS present_count
    FROM roster_marks
    GROUP BY session_id
),
search_matches AS (
    SELECT DISTINCT session_id FROM roster
    WHERE $10::text IS NOT NULL AND (name ILIKE $10 OR roll_number ILIKE $10)
)
SELECT ss.id, ss.created_at, ss.track, ss.class_label, ss.session_time_raw,
       COALESCE(ra.roster_size, 0) AS roster_size,
       COALESCE(ra.marked_count, 0) AS marked_count,
       COALESCE(ra.present_count, 0) AS present_count
FROM scoped_sessions ss
LEFT JOIN roster_agg ra ON ra.session_id = ss.id
WHERE
  ($10::text IS NULL OR ss.id IN (SELECT session_id FROM search_matches))
  AND (
    $11::text IS NULL
    OR ($11 = 'no_roster' AND COALESCE(ra.roster_size, 0) = 0)
    OR ($11 = 'not_started' AND COALESCE(ra.roster_size, 0) > 0 AND COALESCE(ra.marked_count, 0) = 0)
    OR ($11 = 'partial' AND COALESCE(ra.roster_size, 0) > 0 AND COALESCE(ra.marked_count, 0) > 0 AND ra.marked_count < ra.roster_size)
    OR ($11 = 'complete' AND COALESCE(ra.roster_size, 0) > 0 AND ra.marked_count = ra.roster_size)
  )
ORDER BY ss.created_at DESC
LIMIT $12
OFFSET $13
"#;

pub async fn query_session_records(
    db: &sqlx::PgPool,
    auth: &AuthenticatedAdmin,
    filters: &SessionRecordFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionRecordStats>> {
    let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));
    let rows: Vec<SessionRecordStats> = sqlx::query_as(SESSION_RECORDS_SQL)
        .bind(&auth.role)
        .bind(auth.id)
        .bind(filters.location_id)
        .bind(filters.date_start)
        .bind(filters.date_end)
        .bind(&filters.batch_ids)
        .bind(&filters.class_labels)
        .bind(&filters.tracks)
        .bind(&filters.time_slots)
        .bind(&search_pattern)
        .bind(&filters.marking_status)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(rows)
}

/// Same roster-progress computation as `query_session_records`, but for one
/// already-resolved session (used by `get_session`, where a full role-scoped
/// CTE scan would be overkill for a single row whose ownership is already
/// verified).
pub async fn session_record_stats_for(
    db: &sqlx::PgPool,
    session: &Session,
) -> Result<SessionRecordStats> {
    let (track, class_label, session_time_raw): (Option<String>, Option<String>, Option<String>) =
        if let Some(excel_batch_id) = session.excel_batch_id {
            sqlx::query_as(
                "SELECT track, class_label, session_time_raw FROM excel_batches WHERE id = $1",
            )
            .bind(excel_batch_id)
            .fetch_optional(db)
            .await?
            .unwrap_or((None, None, None))
        } else if let Some(location_id) = session.location_id {
            let name: Option<String> =
                sqlx::query_scalar("SELECT name FROM locations WHERE id = $1")
                    .bind(location_id)
                    .fetch_optional(db)
                    .await?;
            (None, name, None)
        } else {
            (None, None, None)
        };

    let has_roster = session.batch_id.is_some() || session.excel_batch_id.is_some();
    let (roster_size, marked_count, present_count): (i64, i64, i64) = if has_roster {
        sqlx::query_as(
            "WITH roster AS ( \
                 SELECT roll_number FROM students WHERE batch_id = $1 \
                 UNION ALL \
                 SELECT roll_number FROM excel_batch_students WHERE excel_batch_id = $2 \
             ), marks AS ( \
                 SELECT r.roll_number, a.status AS marked_status \
                 FROM roster r \
                 LEFT JOIN attendances a ON a.session_id = $3 AND upper(a.roll_number) = upper(r.roll_number) \
             ) \
             SELECT COUNT(*), COUNT(*) FILTER (WHERE marked_status IS NOT NULL), \
                    COUNT(*) FILTER (WHERE marked_status = 'present') \
             FROM marks",
        )
        .bind(session.batch_id)
        .bind(session.excel_batch_id)
        .bind(session.id)
        .fetch_one(db)
        .await?
    } else {
        (0, 0, 0)
    };

    Ok(SessionRecordStats {
        id: session.id,
        created_at: session.created_at,
        track,
        class_label,
        session_time_raw,
        roster_size,
        marked_count,
        present_count,
    })
}

// =================== Global stats overview ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecordsOverview {
    pub total_records: i64,
    pub filtered_records: i64,
    pub present: i64,
    pub absent: i64,
    pub attendance_percent: f64,
}

/// Global counts for the currently active filters, scanning every matching
/// session (no page-size cap) — this is what lets the Sessions page's stat
/// cards stay accurate even once the list itself is capped for display.
pub async fn get_sessions_stats_overview(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(params): Query<SessionRecordQueryParams>,
) -> Result<impl IntoResponse> {
    let filters = params.into_filters();

    let total_records: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions s \
         WHERE ($1 = 'super_admin' \
                OR EXISTS ( \
                      SELECT 1 FROM session_admins sa WHERE sa.session_id = s.id AND sa.admin_id = $2))",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    let records = query_session_records(&state.db, &auth, &filters, i64::MAX, 0).await?;
    let filtered_records = records.len() as i64;

    let mut present = 0i64;
    let mut absent = 0i64;

    for r in &records {
        present += r.present_count;
        absent += r.roster_size - r.present_count;
    }

    let attendance_percent = if present + absent > 0 {
        (present as f64 / (present + absent) as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(SessionRecordsOverview {
        total_records,
        filtered_records,
        present,
        absent,
        attendance_percent,
    }))
}

// =================== Filtered bulk export ===================

/// Exports every session matching the given filters, resolved server-side
/// with no page-size cap — the counterpart to the checkbox-driven
/// `export_bulk_attendance`, for a colleague who trusts a filter combination
/// (e.g. "Track=DSA") and wants everything it matches without first loading
/// and manually selecting rows.
pub async fn export_sessions_filtered(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<SessionRecordFilterBody>,
) -> Result<impl IntoResponse> {
    let filters = payload.into_filters();
    let records = query_session_records(&state.db, &auth, &filters, i64::MAX, 0).await?;

    if records.is_empty() {
        return Err(crate::error::AppError::NotFound(
            "No sessions match the given filters".to_string(),
        ));
    }

    let session_ids: Vec<Uuid> = records.iter().map(|r| r.id).collect();
    crate::controllers::session::build_sessions_workbook_response(&state, &auth, &session_ids).await
}
