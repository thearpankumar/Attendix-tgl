// Roster resolution shared by the manual-attendance and absent-list
// endpoints. A mentor-marked ("exam") session's roster comes from exactly
// one of two places: the regular `batches`/`students` tables (manually
// created batch) or `excel_batches`/`excel_batch_students` (Excel-upload
// bulk session creation, migration 0003) — the two are mutually exclusive
// per session (`batch_id`/`excel_batch_id`, enforced by a DB check
// constraint). This module lets call sites branch once and get a uniform
// row shape back, instead of duplicating the batch-vs-excel-batch match at
// every query site.

use uuid::Uuid;

use crate::{error::Result, models::Session};

pub enum RosterSource {
    Batch(Uuid),
    ExcelBatch(Uuid),
    None,
}

pub struct RosterStudentRow {
    pub id: Uuid,
    pub roll_number: String,
    pub name: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

/// Raw row shape for a `students` (regular batch) lookup: (id, roll_number,
/// name, college_name, email).
type BatchStudentRow = (Uuid, String, String, Option<String>, Option<String>);

pub fn resolve_roster_source(session: &Session) -> RosterSource {
    match (session.batch_id, session.excel_batch_id) {
        (Some(id), _) => RosterSource::Batch(id),
        (_, Some(id)) => RosterSource::ExcelBatch(id),
        _ => RosterSource::None,
    }
}

/// The roster's display name (`batches.name` or `excel_batches.name`),
/// independent of the full student list — used where only the label is
/// needed (e.g. `RosterSessionInfo.batch_name`).
pub async fn fetch_roster_name(db: &sqlx::PgPool, source: &RosterSource) -> Result<Option<String>> {
    match source {
        RosterSource::Batch(id) => Ok(sqlx::query_scalar("SELECT name FROM batches WHERE id = $1")
            .bind(id)
            .fetch_optional(db)
            .await?),
        RosterSource::ExcelBatch(id) => Ok(sqlx::query_scalar(
            "SELECT name FROM excel_batches WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(db)
        .await?),
        RosterSource::None => Ok(None),
    }
}

pub async fn fetch_roster_students(
    db: &sqlx::PgPool,
    source: &RosterSource,
) -> Result<Vec<RosterStudentRow>> {
    match source {
        RosterSource::Batch(id) => {
            let rows: Vec<BatchStudentRow> = sqlx::query_as(
                "SELECT id, roll_number, name, college_name, email FROM students \
                     WHERE batch_id = $1 ORDER BY position",
            )
            .bind(id)
            .fetch_all(db)
            .await?;
            Ok(rows
                .into_iter()
                .map(
                    |(id, roll_number, name, college_name, email)| RosterStudentRow {
                        id,
                        roll_number,
                        name,
                        college_name,
                        email,
                    },
                )
                .collect())
        }
        RosterSource::ExcelBatch(id) => {
            let rows: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
                "SELECT id, roll_number, name, email FROM excel_batch_students \
                 WHERE excel_batch_id = $1 ORDER BY position",
            )
            .bind(id)
            .fetch_all(db)
            .await?;
            Ok(rows
                .into_iter()
                .map(|(id, roll_number, name, email)| RosterStudentRow {
                    id,
                    roll_number,
                    name,
                    college_name: None,
                    email,
                })
                .collect())
        }
        RosterSource::None => Ok(vec![]),
    }
}

pub async fn find_roster_student(
    db: &sqlx::PgPool,
    source: &RosterSource,
    roll_upper: &str,
) -> Result<Option<RosterStudentRow>> {
    match source {
        RosterSource::Batch(id) => {
            let row: Option<BatchStudentRow> = sqlx::query_as(
                "SELECT id, roll_number, name, college_name, email FROM students \
                     WHERE batch_id = $1 AND upper(roll_number) = $2",
            )
            .bind(id)
            .bind(roll_upper)
            .fetch_optional(db)
            .await?;
            Ok(row.map(
                |(id, roll_number, name, college_name, email)| RosterStudentRow {
                    id,
                    roll_number,
                    name,
                    college_name,
                    email,
                },
            ))
        }
        RosterSource::ExcelBatch(id) => {
            let row: Option<(Uuid, String, String, Option<String>)> = sqlx::query_as(
                "SELECT id, roll_number, name, email FROM excel_batch_students \
                 WHERE excel_batch_id = $1 AND upper(roll_number) = $2",
            )
            .bind(id)
            .bind(roll_upper)
            .fetch_optional(db)
            .await?;
            Ok(row.map(|(id, roll_number, name, email)| RosterStudentRow {
                id,
                roll_number,
                name,
                college_name: None,
                email,
            }))
        }
        RosterSource::None => Ok(None),
    }
}
