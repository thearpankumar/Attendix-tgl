use axum::{
    extract::{Json, Query, State},
    response::IntoResponse,
    Extension,
};
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    Collection,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Attendance, Session},
};

// =================== Flagged Attendance ===================

pub async fn get_flagged_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<FlaggedQuery>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    let mut filter = doc! { "deviceFlag": { "$ne": null } };

    if let Some(session_id_str) = query.session_id {
        let session_id = ObjectId::parse_str(&session_id_str)
            .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

        // Verify session ownership
        sessions_collection
            .find_one(doc! { "_id": session_id, "createdBy": auth.id })
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

        filter.insert("sessionId", session_id);
    }

    let mut cursor = attendances_collection
        .find(filter)
        .sort(doc! { "capturedAt": -1 })
        .limit(100)
        .await?;

    let mut result = Vec::new();
    while cursor.advance().await? {
        let attendance = cursor.deserialize_current()?;
        result.push(attendance);
    }

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct FlaggedQuery {
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttendanceReviewRequest {
    pub reviewed: bool,
    pub review_notes: Option<String>,
}

pub async fn review_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<AttendanceReviewRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    let attendance_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    attendances_collection
        .update_one(
            doc! { "_id": attendance_id },
            doc! {
                "$set": {
                    "flagReviewed": payload.reviewed,
                    "flagReviewedBy": auth.id,
                    "flagReviewedAt": BsonDateTime::now()
                }
            },
        )
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub verified: bool,
}

pub async fn verify_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<VerifyRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());

    let attendance_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    // Get attendance and verify ownership
    let attendance = attendances_collection
        .find_one(doc! { "_id": attendance_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance not found".to_string()))?;

    // Verify session ownership
    sessions_collection
        .find_one(doc! { "_id": attendance.session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    attendances_collection
        .update_one(
            doc! { "_id": attendance_id },
            doc! { "$set": { "verified": payload.verified } },
        )
        .await?;

    Ok(Json(serde_json::json!({
        "message": if payload.verified { "Marked verified" } else { "Marked unverified" },
        "verified": payload.verified
    })))
}

#[derive(Debug, Deserialize)]
pub struct BulkVerifyRequest {
    pub ids: Vec<String>,
    pub verified: bool,
}

pub async fn bulk_verify_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(payload): Json<BulkVerifyRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    if payload.ids.is_empty() {
        return Err(AppError::BadRequest(
            "ids must be a non-empty array".to_string(),
        ));
    }
    if payload.ids.len() > 100 {
        return Err(AppError::BadRequest(
            "Cannot bulk-update more than 100 records at once".to_string(),
        ));
    }

    let session_oid = ObjectId::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Verify session ownership
    sessions_collection
        .find_one(doc! { "_id": session_oid, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let ids: Result<Vec<ObjectId>> = payload
        .ids
        .iter()
        .map(|id| {
            ObjectId::parse_str(id)
                .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))
        })
        .collect();
    let ids = ids?;

    let result = attendances_collection
        .update_many(
            doc! { "_id": { "$in": ids }, "sessionId": session_oid },
            doc! { "$set": { "verified": payload.verified } },
        )
        .await?;

    Ok(Json(
        serde_json::json!({ "updated": result.modified_count }),
    ))
}
