use axum::{
    extract::{Json, Query, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::*,
    error::Result,
    middleware::AuthenticatedAdmin,
    models::{Attendance, Batch, Location, Session},
};

// Helper function to get health status string based on score
fn get_health_status(score: i64) -> &'static str {
    if score >= HEALTH_THRESHOLD_GOOD {
        STATUS_ON_TRACK
    } else if score >= HEALTH_THRESHOLD_MEDIUM {
        STATUS_AT_RISK
    } else {
        STATUS_CRITICAL
    }
}

// Helper function to get health status lowercase string
fn get_health_status_lower(score: i64) -> &'static str {
    if score >= HEALTH_THRESHOLD_GOOD {
        HEALTH_STATUS_HEALTHY
    } else if score >= HEALTH_THRESHOLD_MEDIUM {
        HEALTH_STATUS_DEGRADED
    } else {
        HEALTH_STATUS_UNHEALTHY
    }
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub session_id: Option<String>,
    #[serde(rename = "batchId")]
    pub batch_id: Option<String>,
    #[serde(rename = "locationId")]
    pub location_id: Option<String>,
    #[serde(rename = "riskLevel")]
    pub risk_level: Option<String>,
}

// =================== Full Dashboard Stats Structures ===================

#[derive(Debug, Serialize)]
pub struct FullDashboardStats {
    pub pulse: PulseMetrics,
    pub charts: ChartsData,
    pub worklists: WorklistsData,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
}

#[derive(Debug, Serialize)]
pub struct PulseMetrics {
    pub eligibility: PulseMetric,
    pub integrity: IntegrityMetric,
    pub turnout: PulseMetric,
    pub quarantine: QuarantineMetric,
}

#[derive(Debug, Serialize)]
pub struct PulseMetric {
    pub value: i64,
    pub target: i64,
    pub delta: i64,
    #[serde(rename = "deltaType")]
    pub delta_type: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct IntegrityMetric {
    pub value: i64,
    pub target: i64,
    pub delta: i64,
    #[serde(rename = "deltaType")]
    pub delta_type: String,
    pub status: String,
    pub components: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct QuarantineMetric {
    pub count: i64,
    pub status: String,
    /// Substitute for real alerting (no email/webhook/websocket-push infra
    /// exists) — lets the sidebar badge visually distinguish "something
    /// urgent needs review" from an ordinary backlog.
    #[serde(rename = "hasHighSeverityUnreviewed")]
    pub has_high_severity_unreviewed: bool,
}

#[derive(Debug, Serialize)]
pub struct ChartsData {
    pub funnel: FunnelData,
    #[serde(rename = "integrityBreakdown")]
    pub integrity_breakdown: IntegrityBreakdown,
    #[serde(rename = "systemHealth")]
    pub system_health: SystemHealthChart,
    #[serde(rename = "weeklyTrends")]
    pub weekly_trends: Vec<WeeklyTrend>,
}

#[derive(Debug, Serialize)]
pub struct FunnelData {
    pub total: i64,
    #[serde(rename = "onTrack")]
    pub on_track: FunnelStep,
    #[serde(rename = "atRisk")]
    pub at_risk: FunnelStep,
    pub disqualified: FunnelStep,
}

#[derive(Debug, Serialize)]
pub struct FunnelStep {
    pub count: i64,
    pub percentage: i64,
}

#[derive(Debug, Serialize)]
pub struct IntegrityBreakdown {
    #[serde(rename = "totalCheckins")]
    pub total_checkins: i64,
    #[serde(rename = "flaggedAnomalies")]
    pub flagged_anomalies: i64,
    pub score: i64,
    pub flags: IntegrityFlags,
}

#[derive(Debug, Serialize)]
pub struct IntegrityFlags {
    #[serde(rename = "gpsViolations")]
    pub gps_violations: FlagDetail,
    #[serde(rename = "deviceAnomalies")]
    pub device_anomalies: FlagDetail,
}

#[derive(Debug, Serialize)]
pub struct FlagDetail {
    pub count: i64,
    pub percentage: i64,
}

#[derive(Debug, Serialize)]
pub struct SystemHealthChart {
    pub score: i64,
    pub status: String,
    #[serde(rename = "healthStatus")]
    pub health_status: String,
    pub components: Option<serde_json::Value>,
    pub summary: SystemHealthSummary,
}

#[derive(Debug, Serialize)]
pub struct SystemHealthSummary {
    #[serde(rename = "healthyComponents")]
    pub healthy_components: i64,
    #[serde(rename = "totalComponents")]
    pub total_components: i64,
}

#[derive(Debug, Serialize)]
pub struct WeeklyTrend {
    pub date: String,
    pub day: String,
    pub rate: i64,
}

#[derive(Debug, Serialize)]
pub struct WorklistsData {
    #[serde(rename = "rescueList")]
    pub rescue_list: Vec<RescueItem>,
    #[serde(rename = "rescueCount")]
    pub rescue_count: i64,
    #[serde(rename = "highBatches")]
    pub high_batches: Vec<HighBatch>,
    #[serde(rename = "highBatchesCount")]
    pub high_batches_count: i64,
    #[serde(rename = "lowBatches")]
    pub low_batches: Vec<LowBatch>,
    #[serde(rename = "lowBatchesCount")]
    pub low_batches_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RescueItem {
    #[serde(rename = "rollNo")]
    pub roll_no: String,
    pub name: String,
    pub batch: String,
    pub attendance: i64,
    pub trend: String,
}

#[derive(Debug, Serialize)]
pub struct LowBatch {
    pub name: String,
    pub center: String,
    pub trainer: String,
    pub attendance: i64,
}

#[derive(Debug, Serialize)]
pub struct HighBatch {
    pub name: String,
    pub center: String,
    pub trainer: String,
    pub attendance: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct IdName {
    id: Uuid,
    name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct BatchIdCount {
    batch_id: Uuid,
    cnt: i64,
}

pub async fn get_dashboard_stats(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<DashboardQuery>,
) -> Result<impl IntoResponse> {
    // Build session filter query dynamically. Any super-admin sees every
    // session; a mentor only ones they created themselves (dashboards are
    // not part of the mentor app, but this keeps the scoping consistent with
    // every other admin-role query in this file).
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM sessions WHERE (");
    qb.push_bind(auth.role.clone());
    qb.push(" = 'super_admin' OR created_by = ");
    qb.push_bind(auth.id);
    qb.push(")");

    if let Some(batch_id_str) = &query.batch_id {
        if batch_id_str != "all" {
            if let Ok(batch_uuid) = Uuid::parse_str(batch_id_str) {
                qb.push(" AND batch_id = ");
                qb.push_bind(batch_uuid);
            }
        }
    }

    if let Some(location_id_str) = &query.location_id {
        if location_id_str != "all" {
            if let Ok(location_uuid) = Uuid::parse_str(location_id_str) {
                qb.push(" AND location_id = ");
                qb.push_bind(location_uuid);
            }
        }
    }

    if let (Some(start_str), Some(end_str)) = (&query.start_date, &query.end_date) {
        if let (Ok(start), Ok(end)) = (
            DateTime::parse_from_rfc3339(start_str),
            DateTime::parse_from_rfc3339(end_str),
        ) {
            qb.push(" AND created_at >= ");
            qb.push_bind(start.with_timezone(&Utc));
            qb.push(" AND created_at <= ");
            qb.push_bind(end.with_timezone(&Utc));
        }
    }

    let sessions: Vec<Session> = qb.build_query_as().fetch_all(&state.db).await?;

    let mut session_ids: Vec<Uuid> = Vec::new();
    let mut session_batch_map: std::collections::HashMap<Uuid, Option<Uuid>> =
        std::collections::HashMap::new();

    for session in &sessions {
        session_ids.push(session.id);
        session_batch_map.insert(session.id, session.batch_id);
    }

    // If no sessions exist, return default zeroed payload
    if session_ids.is_empty() {
        let system_health = crate::services::system_health::get_system_health(
            &state.db,
            Some(&state.redis),
            &state.storage,
        )
        .await?;
        let health_score = system_health.overall_score as i64;
        let health_status = get_health_status(health_score);
        let health_status_lower = get_health_status_lower(health_score);

        return Ok(Json(FullDashboardStats {
            pulse: PulseMetrics {
                eligibility: PulseMetric {
                    value: 0,
                    target: 90,
                    delta: 0,
                    delta_type: DELTA_TYPE_UP.to_string(),
                    status: STATUS_ON_TRACK.to_string(),
                },
                integrity: IntegrityMetric {
                    value: health_score,
                    target: 100,
                    delta: 0,
                    delta_type: DELTA_TYPE_RIGHT.to_string(),
                    status: health_status.to_string(),
                    components: Some(serde_json::json!({
                        "aiModel": { "name": "AI Model", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "backend": { "name": "Backend", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "studentContainers": { "name": "Student Containers", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "adminService": { "name": "Admin Service", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 }
                    })),
                },
                turnout: PulseMetric {
                    value: 0,
                    target: 85,
                    delta: 0,
                    delta_type: DELTA_TYPE_DOWN.to_string(),
                    status: STATUS_AT_RISK.to_string(),
                },
                quarantine: QuarantineMetric {
                    count: 0,
                    status: STATUS_ON_TRACK.to_string(),
                    has_high_severity_unreviewed: false,
                },
            },
            charts: ChartsData {
                funnel: FunnelData {
                    total: 0,
                    on_track: FunnelStep {
                        count: 0,
                        percentage: 0,
                    },
                    at_risk: FunnelStep {
                        count: 0,
                        percentage: 0,
                    },
                    disqualified: FunnelStep {
                        count: 0,
                        percentage: 0,
                    },
                },
                integrity_breakdown: IntegrityBreakdown {
                    total_checkins: 0,
                    flagged_anomalies: 0,
                    score: 0,
                    flags: IntegrityFlags {
                        gps_violations: FlagDetail {
                            count: 0,
                            percentage: 0,
                        },
                        device_anomalies: FlagDetail {
                            count: 0,
                            percentage: 0,
                        },
                    },
                },
                system_health: SystemHealthChart {
                    score: health_score,
                    status: health_status.to_string(),
                    health_status: health_status_lower.to_string(),
                    components: Some(serde_json::json!({
                        "aiModel": { "name": "AI Model", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "backend": { "name": "Backend", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "studentContainers": { "name": "Student Containers", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                        "adminService": { "name": "Admin Service", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 }
                    })),
                    summary: SystemHealthSummary {
                        healthy_components: if health_score >= HEALTH_THRESHOLD_GOOD {
                            4
                        } else if health_score >= HEALTH_THRESHOLD_MEDIUM {
                            2
                        } else {
                            0
                        },
                        total_components: 4,
                    },
                },
                weekly_trends: vec![],
            },
            worklists: WorklistsData {
                rescue_list: vec![],
                rescue_count: 0,
                high_batches: vec![],
                high_batches_count: 0,
                low_batches: vec![],
                low_batches_count: 0,
            },
            last_updated: Utc::now().to_rfc3339(),
        }));
    }

    // Get total checkins, device flags, gps flags, and quarantine counts
    let total_checkins: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = ANY($1)")
            .bind(&session_ids)
            .fetch_one(&state.db)
            .await?;

    let device_flags_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = ANY($1) AND device_flag IS NOT NULL",
    )
    .bind(&session_ids)
    .fetch_one(&state.db)
    .await?;

    let gps_flags_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = ANY($1) AND distance_from_location > 100",
    )
    .bind(&session_ids)
    .fetch_one(&state.db)
    .await?;

    // Matches the unified review queue's own WHERE clause (admin_security::
    // get_flag_queue) — this used to be `verified = false AND device_flag IS
    // NOT NULL`, a third, slightly different definition of "needs review"
    // than the queue and the per-session summary each used, so the sidebar
    // badge, the queue list, and the security summary could all disagree on
    // how many flags were outstanding.
    let quarantine_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = ANY($1) \
         AND flag_reviewed = false AND (flagged = true OR device_flag IS NOT NULL)",
    )
    .bind(&session_ids)
    .fetch_one(&state.db)
    .await?;

    let has_high_severity_unreviewed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendances WHERE session_id = ANY($1) \
         AND flag_reviewed = false AND flag_severity = 'high')",
    )
    .bind(&session_ids)
    .fetch_one(&state.db)
    .await?;

    let total_anomalies = device_flags_count + gps_flags_count;
    let integrity_score = if total_checkins > 0 {
        ((total_checkins - total_anomalies) * 100 / total_checkins).min(100)
    } else {
        100
    };

    // Get batch session counts for expected checkins calculation
    let mut batch_session_counts: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    for batch_id in session_batch_map.values().flatten() {
        let key = batch_id.to_string();
        *batch_session_counts.entry(key).or_insert(0) += 1;
    }

    // Get unique students with their checkin counts
    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = ANY($1)")
            .bind(&session_ids)
            .fetch_all(&state.db)
            .await?;

    // Group students by roll number + name + batch
    let mut student_map: std::collections::HashMap<(String, String, String, String), i64> =
        std::collections::HashMap::new();

    for attendance in &attendances {
        let batch_name = session_batch_map
            .get(&attendance.session_id)
            .and_then(|batch_id_opt| batch_id_opt.map(|id| id.to_string()))
            .unwrap_or_default();
        let key = (
            attendance.roll_number.to_uppercase(),
            attendance.student_name.clone(),
            batch_name.clone(),
            batch_name,
        );
        *student_map.entry(key).or_insert(0) += 1;
    }

    let mut on_track_count: i64 = 0;
    let mut at_risk_count: i64 = 0;
    let mut disqualified_count: i64 = 0;
    let mut rescue_list: Vec<RescueItem> = Vec::new();

    // Get batch names
    let batch_ids: Vec<Uuid> = session_batch_map.values().filter_map(|b| *b).collect();

    let mut batch_names: std::collections::HashMap<Uuid, String> = std::collections::HashMap::new();
    if !batch_ids.is_empty() {
        let rows: Vec<IdName> = sqlx::query_as("SELECT id, name FROM batches WHERE id = ANY($1)")
            .bind(&batch_ids)
            .fetch_all(&state.db)
            .await?;
        for row in rows {
            batch_names.insert(row.id, row.name);
        }
    }

    for ((roll_no, name, batch_id_str, _), checkins) in &student_map {
        let expected_checkins = batch_session_counts
            .get(batch_id_str)
            .copied()
            .unwrap_or(10);
        let percentage = if expected_checkins > 0 {
            (checkins * 100 / expected_checkins).min(100)
        } else {
            0
        };

        let is_low_risk = percentage >= RISK_THRESHOLD_LOW;
        let is_medium_risk = (RISK_THRESHOLD_MEDIUM..RISK_THRESHOLD_LOW).contains(&percentage);
        let is_high_risk = percentage < RISK_THRESHOLD_MEDIUM;

        if is_low_risk {
            on_track_count += 1;
        } else if is_medium_risk {
            at_risk_count += 1;
        } else {
            disqualified_count += 1;
        }

        if is_medium_risk || is_high_risk {
            let batch_name = Uuid::parse_str(batch_id_str)
                .ok()
                .and_then(|uuid| batch_names.get(&uuid).cloned())
                .unwrap_or_else(|| "N/A".to_string());

            rescue_list.push(RescueItem {
                roll_no: roll_no.clone(),
                name: name.clone(),
                batch: batch_name,
                attendance: percentage,
                trend: if is_low_risk {
                    DELTA_TYPE_UP
                } else if is_medium_risk {
                    DELTA_TYPE_RIGHT
                } else {
                    DELTA_TYPE_DOWN
                }
                .to_string(),
            });
        }
    }

    // Sort rescue list by attendance (lowest first) and take top 10
    rescue_list.sort_by_key(|a| a.attendance);
    let rescue_count = rescue_list.len() as i64;
    rescue_list.truncate(DASHBOARD_RESCUE_LIST_LIMIT);

    let total_students = on_track_count + at_risk_count + disqualified_count;
    let avg_eligibility = if total_students > 0 {
        (on_track_count + at_risk_count) * 100 / total_students
    } else {
        0
    };

    // Weekly trends - get daily attendance for last 7 days
    let week_ago = Utc::now() - chrono::Duration::days(7);
    let weekly_attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = ANY($1) AND captured_at >= $2",
    )
    .bind(&session_ids)
    .bind(week_ago)
    .fetch_all(&state.db)
    .await?;

    let mut daily_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for attendance in &weekly_attendances {
        let date_str = attendance.captured_at.format("%b %d").to_string();
        *daily_counts.entry(date_str).or_insert(0) += 1;
    }

    let mut weekly_trends: Vec<WeeklyTrend> = daily_counts
        .into_iter()
        .map(|(date, count)| {
            let rate = if total_students > 0 {
                (count * 100 / total_students).min(100)
            } else {
                0
            };
            WeeklyTrend {
                day: date.clone(),
                date,
                rate,
            }
        })
        .collect();
    weekly_trends.sort_by(|a, b| a.date.cmp(&b.date));

    if weekly_trends.is_empty() {
        weekly_trends.push(WeeklyTrend {
            date: "Today".to_string(),
            day: "Today".to_string(),
            rate: 0,
        });
    }

    // Low / high engagement batches
    let batches: Vec<Batch> =
        sqlx::query_as("SELECT * FROM batches WHERE ($1 = 'super_admin' OR created_by = $2)")
            .bind(&auth.role)
            .bind(auth.id)
            .fetch_all(&state.db)
            .await?;

    let batch_ids_for_student_count: Vec<Uuid> = batches.iter().map(|b| b.id).collect();
    let mut batch_student_counts: std::collections::HashMap<Uuid, i64> =
        std::collections::HashMap::new();
    if !batch_ids_for_student_count.is_empty() {
        let rows: Vec<BatchIdCount> = sqlx::query_as(
            "SELECT batch_id, COUNT(*) as cnt FROM students WHERE batch_id = ANY($1) GROUP BY batch_id",
        )
        .bind(&batch_ids_for_student_count)
        .fetch_all(&state.db)
        .await?;
        for row in rows {
            batch_student_counts.insert(row.batch_id, row.cnt);
        }
    }

    let mut low_batches: Vec<LowBatch> = Vec::new();
    let mut high_batches: Vec<HighBatch> = Vec::new();
    for batch in &batches {
        let batch_id = batch.id;
        // Get sessions for this batch
        let batch_session_count = batch_session_counts
            .get(&batch_id.to_string())
            .copied()
            .unwrap_or(0);
        if batch_session_count == 0 {
            continue;
        }

        // Count attendance for this batch's sessions
        let batch_session_ids: Vec<Uuid> = session_batch_map
            .iter()
            .filter(|(_, bid)| bid.as_ref() == Some(&batch_id))
            .map(|(sid, _)| *sid)
            .collect();

        if batch_session_ids.is_empty() {
            continue;
        }

        let batch_checkins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = ANY($1)")
                .bind(&batch_session_ids)
                .fetch_one(&state.db)
                .await?;

        let student_count = batch_student_counts.get(&batch_id).copied().unwrap_or(0);
        let total_possible = student_count * batch_session_count;
        let attendance_pct = if total_possible > 0 {
            (batch_checkins * 100 / total_possible).min(100)
        } else {
            0
        };

        if attendance_pct < 80 {
            low_batches.push(LowBatch {
                name: batch.name.clone(),
                center: "Main Campus".to_string(),
                trainer: "System".to_string(),
                attendance: attendance_pct,
            });
        } else if attendance_pct >= DASHBOARD_HIGH_ENGAGEMENT_THRESHOLD {
            high_batches.push(HighBatch {
                name: batch.name.clone(),
                center: "Main Campus".to_string(),
                trainer: "System".to_string(),
                attendance: attendance_pct,
            });
        }
    }

    low_batches.sort_by_key(|a| a.attendance);
    let low_batches_count = low_batches.len() as i64;
    low_batches.truncate(DASHBOARD_LOW_BATCHES_LIMIT);

    high_batches.sort_by_key(|a| std::cmp::Reverse(a.attendance));
    let high_batches_count = high_batches.len() as i64;
    high_batches.truncate(DASHBOARD_HIGH_BATCHES_LIMIT);

    // System health
    let system_health = crate::services::system_health::get_system_health(
        &state.db,
        Some(&state.redis),
        &state.storage,
    )
    .await?;
    let health_score = system_health.overall_score as i64;
    let health_status = get_health_status(health_score);
    let health_status_lower = get_health_status_lower(health_score);

    let turnout_rate = weekly_trends.last().map(|t| t.rate).unwrap_or(0);

    Ok(Json(FullDashboardStats {
        pulse: PulseMetrics {
            eligibility: PulseMetric {
                value: avg_eligibility,
                target: 90,
                delta: 2,
                delta_type: DELTA_TYPE_UP.to_string(),
                status: if avg_eligibility >= RISK_THRESHOLD_LOW {
                    STATUS_ON_TRACK
                } else {
                    STATUS_AT_RISK
                }
                .to_string(),
            },
            integrity: IntegrityMetric {
                value: health_score,
                target: 100,
                delta: 0,
                delta_type: DELTA_TYPE_RIGHT.to_string(),
                status: health_status.to_string(),
                components: Some(serde_json::json!({
                    "aiModel": { "name": "AI Model", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "backend": { "name": "Backend", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "studentContainers": { "name": "Student Containers", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "adminService": { "name": "Admin Service", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 }
                })),
            },
            turnout: PulseMetric {
                value: turnout_rate,
                target: RISK_THRESHOLD_LOW,
                delta: 0,
                delta_type: DELTA_TYPE_RIGHT.to_string(),
                status: STATUS_ON_TRACK.to_string(),
            },
            quarantine: QuarantineMetric {
                count: quarantine_count,
                status: if quarantine_count > 0 {
                    STATUS_CRITICAL
                } else {
                    STATUS_ON_TRACK
                }
                .to_string(),
                has_high_severity_unreviewed,
            },
        },
        charts: ChartsData {
            funnel: FunnelData {
                total: total_students,
                on_track: FunnelStep {
                    count: on_track_count,
                    percentage: if total_students > 0 {
                        on_track_count * 100 / total_students
                    } else {
                        0
                    },
                },
                at_risk: FunnelStep {
                    count: at_risk_count,
                    percentage: if total_students > 0 {
                        at_risk_count * 100 / total_students
                    } else {
                        0
                    },
                },
                disqualified: FunnelStep {
                    count: disqualified_count,
                    percentage: if total_students > 0 {
                        disqualified_count * 100 / total_students
                    } else {
                        0
                    },
                },
            },
            integrity_breakdown: IntegrityBreakdown {
                total_checkins,
                flagged_anomalies: total_anomalies,
                score: integrity_score,
                flags: IntegrityFlags {
                    gps_violations: FlagDetail {
                        count: gps_flags_count,
                        percentage: if total_checkins > 0 {
                            gps_flags_count * 100 / total_checkins
                        } else {
                            0
                        },
                    },
                    device_anomalies: FlagDetail {
                        count: device_flags_count,
                        percentage: if total_checkins > 0 {
                            device_flags_count * 100 / total_checkins
                        } else {
                            0
                        },
                    },
                },
            },
            system_health: SystemHealthChart {
                score: health_score,
                status: health_status.to_string(),
                health_status: health_status_lower.to_string(),
                components: Some(serde_json::json!({
                    "aiModel": { "name": "AI Model", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "backend": { "name": "Backend", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "studentContainers": { "name": "Student Containers", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 },
                    "adminService": { "name": "Admin Service", "healthy": health_score >= HEALTH_THRESHOLD_GOOD, "score": health_score, "weight": 25 }
                })),
                summary: SystemHealthSummary {
                    healthy_components: if health_score >= HEALTH_THRESHOLD_GOOD {
                        4
                    } else if health_score >= HEALTH_THRESHOLD_MEDIUM {
                        2
                    } else {
                        0
                    },
                    total_components: 4,
                },
            },
            weekly_trends,
        },
        worklists: WorklistsData {
            rescue_list,
            rescue_count,
            high_batches,
            high_batches_count,
            low_batches,
            low_batches_count,
        },
        last_updated: Utc::now().to_rfc3339(),
    }))
}

/// Instance-wide infrastructure health (database, Redis, object storage).
///
/// Intentionally not scoped to the calling admin — there is no per-admin view
/// of whether Postgres is reachable. Authentication alone is the correct gate
/// here, unlike the handlers that read or mutate per-admin records.
pub async fn get_system_health(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    use crate::services::system_health::get_system_health as check_system_health;

    let health = check_system_health(&state.db, Some(&state.redis), &state.storage).await?;
    Ok(Json(health))
}

// Dashboard Filters
#[derive(Debug, Serialize)]
pub struct DashboardFilters {
    pub batches: Vec<FilterOption>,
    pub centers: Vec<FilterOption>,
    pub timeframes: Vec<String>,
    pub risk_levels: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FilterOption {
    pub value: String,
    pub label: String,
}

pub async fn get_dashboard_filters(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let batches: Vec<Batch> = sqlx::query_as(
        "SELECT * FROM batches WHERE ($1 = 'super_admin' OR created_by = $2) ORDER BY name",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    let mut batches_out = vec![FilterOption {
        value: "all".to_string(),
        label: "All Batches".to_string(),
    }];
    for batch in batches {
        batches_out.push(FilterOption {
            value: batch.id.to_string(),
            label: batch.name,
        });
    }

    let locations: Vec<Location> = sqlx::query_as(
        "SELECT * FROM locations WHERE ($1 = 'super_admin' OR created_by = $2) ORDER BY name",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    let mut centers = vec![FilterOption {
        value: "all".to_string(),
        label: "All Centers".to_string(),
    }];
    for location in locations {
        centers.push(FilterOption {
            value: location.id.to_string(),
            label: location.name,
        });
    }

    let today = Utc::now();
    let day_of_week = today.weekday().num_days_from_monday() as i64;
    let start_of_week = today - chrono::Duration::days(day_of_week);
    let end_of_week = start_of_week + chrono::Duration::days(6);

    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let short_months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let yesterday = today - chrono::Duration::days(1);

    let timeframes = vec![
        format!(
            "This Week ({} {} - {} {})",
            short_months[start_of_week.month() as usize - 1],
            start_of_week.day(),
            short_months[end_of_week.month() as usize - 1],
            end_of_week.day()
        ),
        format!(
            "Today ({} {})",
            short_months[today.month() as usize - 1],
            today.day()
        ),
        format!(
            "Yesterday ({} {})",
            short_months[yesterday.month() as usize - 1],
            yesterday.day()
        ),
        format!("This Month ({})", months[today.month() as usize - 1]),
    ];

    let risk_levels = vec![
        "All Levels".to_string(),
        "High Risk".to_string(),
        "Medium Risk".to_string(),
        "Low Risk".to_string(),
    ];

    Ok(Json(DashboardFilters {
        batches: batches_out,
        centers,
        timeframes,
        risk_levels,
    }))
}

// Recent Activity
#[derive(Debug, Serialize)]
pub struct RecentActivity {
    pub student_name: String,
    pub roll_number: String,
    pub location_name: String,
    pub captured_at: DateTime<Utc>,
    pub verified: bool,
}

pub async fn get_recent_activity(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    // Every session visible to this admin: all of them for a super-admin,
    // only ones they created for a mentor.
    let sessions: Vec<Session> =
        sqlx::query_as("SELECT * FROM sessions WHERE ($1 = 'super_admin' OR created_by = $2)")
            .bind(&auth.role)
            .bind(auth.id)
            .fetch_all(&state.db)
            .await?;
    let mut session_map: std::collections::HashMap<Uuid, Option<Uuid>> =
        std::collections::HashMap::new();
    for session in &sessions {
        session_map.insert(session.id, session.location_id);
    }

    if session_map.is_empty() {
        return Ok(Json::<Vec<RecentActivity>>(vec![]));
    }

    let session_ids: Vec<Uuid> = session_map.keys().cloned().collect();

    // Pre-fetch all unique location IDs to avoid N+1 queries. Exam sessions
    // have no location — filtered out here, not queried.
    let location_ids: std::collections::HashSet<Uuid> =
        session_map.values().filter_map(|v| *v).collect();
    let location_ids_vec: Vec<Uuid> = location_ids.into_iter().collect();

    let locations: Vec<Location> = sqlx::query_as("SELECT * FROM locations WHERE id = ANY($1)")
        .bind(&location_ids_vec)
        .fetch_all(&state.db)
        .await?;

    let mut location_names: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::new();
    for loc in locations {
        location_names.insert(loc.id, loc.name);
    }

    // Get recent attendance for these sessions
    let attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = ANY($1) ORDER BY captured_at DESC LIMIT $2",
    )
    .bind(&session_ids)
    .bind(DASHBOARD_QUARANTINE_LIST_LIMIT)
    .fetch_all(&state.db)
    .await?;

    let mut activities = Vec::new();
    for attendance in attendances {
        // Get location name from pre-fetched HashMap
        let location_name = session_map
            .get(&attendance.session_id)
            .and_then(|loc_id| loc_id.as_ref())
            .and_then(|loc_id| location_names.get(loc_id).cloned())
            .unwrap_or_else(|| "Unknown".to_string());

        activities.push(RecentActivity {
            student_name: attendance.student_name,
            roll_number: attendance.roll_number,
            location_name,
            captured_at: attendance.captured_at,
            verified: attendance.verified,
        });
    }

    Ok(Json(activities))
}

// Attendance Series
#[derive(Debug, Deserialize)]
pub struct AttendanceSeriesQuery {
    pub days: Option<i64>,
    pub location_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AttendanceSeriesItem {
    pub date: String,
    pub location: String,
    pub session: String,
    pub count: i64,
}

pub async fn get_attendance_series(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<AttendanceSeriesQuery>,
) -> Result<impl IntoResponse> {
    let days = query.days.unwrap_or(180).min(730);
    let from = Utc::now() - chrono::Duration::days(days);

    // Every location visible to this admin.
    let locations: Vec<Location> =
        sqlx::query_as("SELECT * FROM locations WHERE ($1 = 'super_admin' OR created_by = $2)")
            .bind(&auth.role)
            .bind(auth.id)
            .fetch_all(&state.db)
            .await?;
    let mut location_ids: Vec<Uuid> = Vec::new();
    let mut location_names: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::new();
    for loc in locations {
        location_ids.push(loc.id);
        location_names.insert(loc.id, loc.name);
    }

    if location_ids.is_empty() {
        return Ok(Json::<Vec<AttendanceSeriesItem>>(vec![]));
    }

    // Get sessions for these locations
    let sessions: Vec<Session> =
        sqlx::query_as("SELECT * FROM sessions WHERE location_id = ANY($1)")
            .bind(&location_ids)
            .fetch_all(&state.db)
            .await?;
    let mut session_map: std::collections::HashMap<Uuid, (Option<Uuid>, String)> =
        std::collections::HashMap::new();
    let mut session_ids: Vec<Uuid> = Vec::new();
    for session in sessions {
        session_ids.push(session.id);
        session_map.insert(
            session.id,
            (
                session.location_id,
                session.description.unwrap_or_else(|| "Session".to_string()),
            ),
        );
    }

    if session_ids.is_empty() {
        return Ok(Json::<Vec<AttendanceSeriesItem>>(vec![]));
    }

    // Get attendance
    let attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = ANY($1) AND captured_at >= $2 ORDER BY captured_at ASC",
    )
    .bind(&session_ids)
    .bind(from)
    .fetch_all(&state.db)
    .await?;

    let mut result = Vec::new();
    for attendance in attendances {
        if let Some((location_id, description)) = session_map.get(&attendance.session_id) {
            let location = location_id
                .and_then(|id| location_names.get(&id))
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let date = attendance.captured_at.format("%Y-%m-%d").to_string();
            result.push(AttendanceSeriesItem {
                date,
                location,
                session: description.clone(),
                count: 1,
            });
        }
    }

    Ok(Json(result))
}

// Sessions by Date
#[derive(Debug, Deserialize)]
pub struct SessionsByDateQuery {
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct SessionByDateItem {
    pub session_id: String,
    pub session: String,
    pub description: String,
    pub location: String,
    pub time: DateTime<Utc>,
    pub count: i64,
    pub date: String,
}

pub async fn get_sessions_by_date(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<SessionsByDateQuery>,
) -> Result<impl IntoResponse> {
    // Every location visible to this admin.
    let locations: Vec<Location> =
        sqlx::query_as("SELECT * FROM locations WHERE ($1 = 'super_admin' OR created_by = $2)")
            .bind(&auth.role)
            .bind(auth.id)
            .fetch_all(&state.db)
            .await?;
    let mut location_ids: Vec<Uuid> = Vec::new();
    let mut location_names: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::new();
    for loc in locations {
        location_ids.push(loc.id);
        location_names.insert(loc.id, loc.name);
    }

    if location_ids.is_empty() {
        return Ok(Json::<Vec<SessionByDateItem>>(vec![]));
    }

    // Get sessions for these locations
    let sessions: Vec<Session> =
        sqlx::query_as("SELECT * FROM sessions WHERE location_id = ANY($1)")
            .bind(&location_ids)
            .fetch_all(&state.db)
            .await?;
    type SessionByDateInfo = (Option<Uuid>, Option<String>, DateTime<Utc>);
    let mut session_map: std::collections::HashMap<Uuid, SessionByDateInfo> =
        std::collections::HashMap::new();
    let mut session_ids: Vec<Uuid> = Vec::new();
    for session in sessions {
        session_ids.push(session.id);
        session_map.insert(
            session.id,
            (session.location_id, session.description, session.created_at),
        );
    }

    if session_ids.is_empty() {
        return Ok(Json::<Vec<SessionByDateItem>>(vec![]));
    }

    // Get attendance for the date
    let date_start_str = format!("{}T00:00:00.000Z", query.date);
    let date_end_str = format!("{}T23:59:59.999Z", query.date);
    let date_start = DateTime::parse_from_rfc3339(&date_start_str)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| crate::error::AppError::BadRequest("Invalid date".to_string()))?;
    let date_end = DateTime::parse_from_rfc3339(&date_end_str)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| crate::error::AppError::BadRequest("Invalid date".to_string()))?;

    let attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = ANY($1) AND captured_at >= $2 AND captured_at < $3",
    )
    .bind(&session_ids)
    .bind(date_start)
    .bind(date_end)
    .fetch_all(&state.db)
    .await?;

    let mut count_map: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    for attendance in &attendances {
        let count = count_map.entry(attendance.session_id).or_insert(0i64);
        *count += 1;
    }

    let mut result = Vec::new();
    for session_id in session_ids {
        if let Some((location_id, description, created_at)) = session_map.get(&session_id) {
            let count = *count_map.get(&session_id).unwrap_or(&0);
            if count > 0 {
                let location = location_id
                    .and_then(|id| location_names.get(&id))
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string());
                result.push(SessionByDateItem {
                    session_id: session_id.to_string(),
                    session: description.clone().unwrap_or_else(|| "Session".to_string()),
                    description: description.clone().unwrap_or_else(|| "Session".to_string()),
                    location,
                    time: *created_at,
                    count,
                    date: query.date.clone(),
                });
            }
        }
    }

    Ok(Json(result))
}
