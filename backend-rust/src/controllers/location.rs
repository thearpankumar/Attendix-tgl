use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    middleware::{
        validators::{validate_request, LocationCreateRequest},
        AuthenticatedAdmin,
    },
    models::{Location, LocationCreate, LocationUpdate},
};

#[derive(Debug, Serialize)]
pub struct LocationResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(rename = "radiusMeters")]
    pub radius_meters: f64,
    pub description: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: bool,
}

impl From<Location> for LocationResponse {
    fn from(loc: Location) -> Self {
        Self {
            id: loc.id.to_string(),
            name: loc.name,
            latitude: loc.latitude,
            longitude: loc.longitude,
            radius_meters: loc.radius_meters,
            description: loc.description,
            is_active: loc.is_active,
        }
    }
}

pub async fn create_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<LocationCreate>,
) -> Result<impl IntoResponse> {
    let validation_req = LocationCreateRequest {
        name: payload.name.clone(),
        latitude: payload.latitude,
        longitude: payload.longitude,
        radius_meters: payload.radius_meters.map(|r| r as i32),
    };
    validate_request(&validation_req)?;

    let radius_meters = payload.radius_meters.unwrap_or(100.0);

    let location = Location {
        id: Uuid::new_v4(),
        name: payload.name,
        latitude: payload.latitude,
        longitude: payload.longitude,
        radius_meters,
        description: payload.description,
        created_by: auth.id,
        is_active: true,
        created_at: Utc::now(),
    };

    location.validate()?;

    let inserted = sqlx::query_as::<_, Location>(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, description, created_by, is_active, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         RETURNING *",
    )
    .bind(location.id)
    .bind(&location.name)
    .bind(location.latitude)
    .bind(location.longitude)
    .bind(location.radius_meters)
    .bind(&location.description)
    .bind(location.created_by)
    .bind(location.is_active)
    .bind(location.created_at)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(LocationResponse::from(inserted))))
}

pub async fn get_locations(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let locations = sqlx::query_as::<_, Location>(
        "SELECT * FROM locations WHERE ($1 = 'super_admin' OR created_by = $2) ORDER BY created_at DESC",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    let response: Vec<LocationResponse> =
        locations.into_iter().map(LocationResponse::from).collect();

    Ok(Json(response))
}

pub async fn get_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let location_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let location = sqlx::query_as::<_, Location>(
        "SELECT * FROM locations WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(location_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    Ok(Json(LocationResponse::from(location)))
}

pub async fn update_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<LocationUpdate>,
) -> Result<impl IntoResponse> {
    let location_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    // A geofence defines what "present" means. Letting a mentor move one they
    // didn't create would falsify that session's attendance wholesale — but
    // any super-admin may, since they're peers, not tenants of each other.
    let existing = sqlx::query_as::<_, Location>(
        "SELECT * FROM locations WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(location_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let touches_geofence = payload.name.is_some()
        || payload.latitude.is_some()
        || payload.longitude.is_some()
        || payload.radius_meters.is_some();

    let merged = Location {
        id: existing.id,
        name: payload.name.unwrap_or(existing.name),
        latitude: payload.latitude.unwrap_or(existing.latitude),
        longitude: payload.longitude.unwrap_or(existing.longitude),
        radius_meters: payload.radius_meters.unwrap_or(existing.radius_meters),
        description: payload.description.or(existing.description),
        created_by: existing.created_by,
        is_active: payload.is_active.unwrap_or(existing.is_active),
        created_at: existing.created_at,
    };

    // Validate the merged record, not just the fields that were sent: a
    // partial PATCH-style body (`{"latitude": 999}`, `{"name": ""}`) used to
    // skip validation entirely because it only ran when name, latitude and
    // longitude were all present.
    // (A deactivate/reactivate-only update is not held to the geofence rules,
    // so a legacy row that predates them can still be retired.)
    if touches_geofence {
        validate_request(&LocationCreateRequest {
            name: merged.name.clone(),
            latitude: merged.latitude,
            longitude: merged.longitude,
            radius_meters: Some(merged.radius_meters as i32),
        })?;
        merged.validate()?;
    }

    let updated = sqlx::query_as::<_, Location>(
        "UPDATE locations SET name = $1, latitude = $2, longitude = $3, radius_meters = $4, \
         description = $5, is_active = $6 WHERE id = $7 RETURNING *",
    )
    .bind(&merged.name)
    .bind(merged.latitude)
    .bind(merged.longitude)
    .bind(merged.radius_meters)
    .bind(&merged.description)
    .bind(merged.is_active)
    .bind(location_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(LocationResponse::from(updated)))
}

/// Rejects a location that doesn't exist or has been deactivated. Used
/// wherever a *new* session or recurring rule is pointed at a location (and
/// when a rule is re-pointed at a different one) — sessions that already
/// reference a deactivated location keep working.
pub(crate) async fn ensure_location_assignable(
    db: impl sqlx::PgExecutor<'_>,
    location_id: Uuid,
) -> Result<()> {
    let active: Option<bool> = sqlx::query_scalar("SELECT is_active FROM locations WHERE id = $1")
        .bind(location_id)
        .fetch_optional(db)
        .await?;
    match active {
        None => Err(AppError::BadRequest("Location not found".to_string())),
        Some(false) => Err(AppError::BadRequest(
            "Location is deactivated; pick an active location".to_string(),
        )),
        Some(true) => Ok(()),
    }
}

fn in_use_message(session_count: i64, rule_count: i64) -> String {
    format!(
        "This location is used by {session_count} session(s) and {rule_count} recurring rule(s) \
         and can't be deleted. Deactivate it instead (set it to inactive) to stop it being used \
         for new sessions while keeping existing attendance history intact."
    )
}

/// Deletes a location that nothing references. A location that sessions or
/// recurring rules still point at can't be removed without orphaning (or
/// destroying) their geofence and attendance history, so it is refused with
/// a 409 that says how many are affected; deactivating it
/// (`PUT /locations/{id}` with `isActive: false`) is the supported way to
/// retire it.
pub async fn delete_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let location_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let mut tx = state.db.begin().await?;

    // Row lock so a concurrent session/rule create can't slip a reference in
    // between the reference check and the delete.
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM locations WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3) FOR UPDATE",
    )
    .bind(location_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let (session_count, rule_count): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM sessions WHERE location_id = $1), \
                (SELECT COUNT(*) FROM recurring_session_rules WHERE location_id = $1)",
    )
    .bind(location_id)
    .fetch_one(&mut *tx)
    .await?;
    if session_count + rule_count > 0 {
        return Err(AppError::Conflict(in_use_message(
            session_count,
            rule_count,
        )));
    }

    match sqlx::query("DELETE FROM locations WHERE id = $1")
        .bind(location_id)
        .execute(&mut *tx)
        .await
    {
        Ok(_) => {}
        // Belt and braces for any other table that grows a reference.
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
            return Err(AppError::Conflict(in_use_message(
                session_count,
                rule_count,
            )));
        }
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "message": "Location deleted successfully"
    })))
}
