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
    if let Some(name) = &payload.name {
        if let Some(lat) = payload.latitude {
            if let Some(lon) = payload.longitude {
                let validation_req = LocationCreateRequest {
                    name: name.clone(),
                    latitude: lat,
                    longitude: lon,
                    radius_meters: payload.radius_meters.map(|r| r as i32),
                };
                validate_request(&validation_req)?;
            }
        }
    }

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

pub async fn delete_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let location_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let result = sqlx::query(
        "DELETE FROM locations WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(location_id)
    .bind(&auth.role)
    .bind(auth.id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Location not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "message": "Location deleted successfully"
    })))
}
