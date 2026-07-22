use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};
use serde::Serialize;
use std::sync::Arc;

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
            id: loc.id.map(|id| id.to_hex()).unwrap_or_default(),
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

    let collection: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let location = Location {
        id: None,
        name: payload.name,
        latitude: payload.latitude,
        longitude: payload.longitude,
        radius_meters: payload.radius_meters.unwrap_or(100.0),
        description: payload.description,
        created_by: auth.id,
        is_active: true,
        created_at: Utc::now(),
    };

    location.validate()?;

    let result = collection.insert_one(&location).await?;
    let location_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    let mut response = LocationResponse::from(location);
    response.id = location_id.to_hex();

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_locations(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let mut cursor = collection
        .find(doc! {})
        .sort(doc! { "createdAt": -1 })
        .await?;
    let mut locations = Vec::new();

    while cursor.advance().await? {
        let loc = cursor.deserialize_current()?;
        locations.push(LocationResponse::from(loc));
    }

    Ok(Json(locations))
}

pub async fn get_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let location_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let location = collection
        .find_one(doc! { "_id": location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    Ok(Json(LocationResponse::from(location)))
}

pub async fn update_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
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

    let collection: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let location_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let mut update_doc = doc! {};

    if let Some(name) = payload.name {
        update_doc.insert("name", name);
    }
    if let Some(lat) = payload.latitude {
        update_doc.insert("latitude", lat);
    }
    if let Some(lon) = payload.longitude {
        update_doc.insert("longitude", lon);
    }
    if let Some(radius) = payload.radius_meters {
        update_doc.insert("radiusMeters", radius);
    }
    if let Some(desc) = payload.description {
        update_doc.insert("description", desc);
    }
    if let Some(active) = payload.is_active {
        update_doc.insert("isActive", active);
    }

    collection
        .update_one(doc! { "_id": location_id }, doc! { "$set": update_doc })
        .await?;

    let location = collection
        .find_one(doc! { "_id": location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    Ok(Json(LocationResponse::from(location)))
}

pub async fn delete_location(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let location_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    collection.delete_one(doc! { "_id": location_id }).await?;

    Ok(StatusCode::NO_CONTENT)
}
