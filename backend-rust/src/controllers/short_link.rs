use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use mongodb::{
    bson::{doc, oid::ObjectId, DateTime as BsonDateTime},
    Collection,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Session, ShortLink},
};

#[derive(Debug, Deserialize)]
pub struct CreateShortLinkRequest {
    pub short_code: Option<String>,
    pub session_id: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShortLinkResponse {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "shortCode")]
    pub short_code: String,
    pub url: String,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<String>,
}

pub async fn create_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateShortLinkRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let collection: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());

    let session_id = payload
        .session_id
        .and_then(|id| ObjectId::parse_str(&id).ok());

    if let Some(sid) = session_id {
        let _session = sessions
            .find_one(doc! { "_id": sid, "createdBy": auth.id })
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found or unauthorized".to_string()))?;

        let existing_link = collection
            .find_one(doc! { "sessionId": sid, "isActive": true })
            .await?;
        if existing_link.is_some() {
            return Err(AppError::BadRequest(
                "Session already has an active short link".to_string(),
            ));
        }

        sessions
            .update_one(
                doc! { "_id": sid },
                doc! { "$set": { "totpEnabled": true } },
            )
            .await?;
    }

    let short_code = payload
        .short_code
        .unwrap_or_else(|| ShortLink::generate_short_code(6));

    let existing = collection
        .find_one(doc! { "shortCode": &short_code })
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest(
            "Short code already exists".to_string(),
        ));
    }

    let expires_at = payload.expires_at.and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
    });

    let short_link = ShortLink {
        id: None,
        short_code: short_code.clone(),
        session_id,
        created_by: auth.id,
        is_active: true,
        expires_at,
        click_count: 0,
        last_clicked_at: None,
        created_at: Utc::now(),
    };

    let result = collection.insert_one(&short_link).await?;
    let link_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(ShortLinkResponse {
            id: link_id.to_hex(),
            short_code: short_code.clone(),
            url: format!("{}/s/{}", state.config.webauthn.origin, short_code),
            session_id: session_id.map(|id| id.to_hex()),
            expires_at: expires_at.map(|d| d.to_rfc3339()),
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ShortLinksQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub session_id: Option<String>,
    pub is_active: Option<String>,
}

pub async fn get_short_links(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<ShortLinksQuery>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let collection: Collection<ShortLink> = db.collection(ShortLink::collection_name());

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);

    let mut filter = doc! {};
    if let Some(sid) = &query.session_id {
        filter.insert("sessionId", sid);
    }
    if let Some(active) = &query.is_active {
        filter.insert("isActive", active == "true");
    }

    let mut cursor = collection
        .find(filter)
        .sort(doc! { "createdAt": -1 })
        .skip(((page - 1) * limit) as u64)
        .limit(limit)
        .await?;
    let mut links = Vec::new();

    while cursor.advance().await? {
        let link = cursor.deserialize_current()?;
        links.push(ShortLinkResponse {
            id: link.id.unwrap().to_hex(),
            short_code: link.short_code.clone(),
            url: format!("{}/s/{}", state.config.webauthn.origin, link.short_code),
            session_id: link.session_id.map(|id| id.to_hex()),
            expires_at: link.expires_at.map(|d| d.to_rfc3339()),
        });
    }

    Ok(Json(links))
}

pub async fn get_short_link_by_code(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let collection: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());

    let link = collection
        .find_one(doc! { "shortCode": short_code.to_lowercase() })
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    if let Some(expires_at) = link.expires_at {
        if expires_at < Utc::now() {
            return Err(AppError::NotFound("Short link has expired".to_string()));
        }
    }

    if !link.is_active {
        return Err(AppError::NotFound("Short link is inactive".to_string()));
    }

    let session_id = link
        .session_id
        .ok_or_else(|| AppError::NotFound("Short link not attached to any session".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active {
        return Err(AppError::BadRequest(
            "Associated session is not active".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "_id": link.id.unwrap().to_hex(),
        "shortCode": link.short_code,
        "sessionId": session_id.to_hex(),
        "isActive": link.is_active,
        "expiresAt": link.expires_at,
        "clickCount": link.click_count,
        "session": {
            "_id": session.id.unwrap().to_hex(),
            "description": session.description,
            "isActive": session.is_active,
            "expiresAt": session.expires_at
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct AttachRequest {
    pub session_id: String,
    pub force: Option<bool>,
}

pub async fn attach_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
    Json(payload): Json<AttachRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());

    let link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase() })
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    let session_id = ObjectId::parse_str(&payload.session_id)
        .map_err(|_| AppError::BadRequest("Invalid session ID format".to_string()))?;

    let _session = sessions
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found or unauthorized".to_string()))?;

    let existing_link = short_links
        .find_one(doc! { "sessionId": session_id, "isActive": true, "_id": { "$ne": link.id } })
        .await?;

    if let Some(existing) = existing_link {
        short_links
            .update_one(
                doc! { "_id": existing.id },
                doc! { "$set": { "sessionId": null, "isActive": false } },
            )
            .await?;
    }

    short_links
        .update_one(
            doc! { "_id": link.id },
            doc! { "$set": { "sessionId": session_id, "isActive": true } },
        )
        .await?;

    sessions
        .update_one(
            doc! { "_id": session_id },
            doc! { "$set": { "totpEnabled": true } },
        )
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Short link attached successfully",
        "shortCode": short_code,
        "sessionId": session_id.to_hex()
    })))
}

pub async fn detach_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let collection: Collection<ShortLink> = db.collection(ShortLink::collection_name());

    let link = collection
        .find_one(doc! { "shortCode": short_code.to_lowercase() })
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    collection
        .update_one(
            doc! { "_id": link.id },
            doc! { "$set": { "sessionId": null, "isActive": false } },
        )
        .await?;

    Ok(Json(
        serde_json::json!({ "message": "Short link detached successfully" }),
    ))
}

pub async fn delete_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let collection: Collection<ShortLink> = db.collection(ShortLink::collection_name());

    let result = collection
        .delete_one(doc! { "shortCode": short_code.to_lowercase() })
        .await?;

    if result.deleted_count == 0 {
        return Err(AppError::NotFound("Short link not found".to_string()));
    }

    Ok(Json(
        serde_json::json!({ "message": "Short link deleted successfully" }),
    ))
}

pub async fn get_available_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );
    let sessions: Collection<Session> = db.collection(Session::collection_name());

    let mut cursor = sessions
        .find(doc! { "isActive": true, "createdBy": auth.id })
        .sort(doc! { "createdAt": -1 })
        .await?;
    let mut results = Vec::new();

    while cursor.advance().await? {
        let session = cursor.deserialize_current()?;
        results.push(serde_json::json!({
            "_id": session.id.unwrap().to_hex(),
            "description": session.description,
            "expiresAt": session.expires_at,
            "createdAt": session.created_at
        }));
    }

    Ok(Json(results))
}

pub async fn resolve_short_link(
    State(state): State<Arc<crate::AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<ShortLink> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(ShortLink::collection_name());

    let link = collection
        .find_one(doc! { "shortCode": &short_code, "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    if link.is_expired() {
        return Err(AppError::NotFound("Short link has expired".to_string()));
    }

    collection
        .update_one(
            doc! { "_id": link.id },
            doc! { "$inc": { "clickCount": 1 }, "$set": { "lastClickedAt": BsonDateTime::now() } },
        )
        .await?;

    let redirect_url = format!("{}/s/{}/session", state.config.webauthn.origin, short_code);

    Ok(([("Location", redirect_url)], StatusCode::FOUND))
}

pub async fn get_short_link_session(
    State(state): State<Arc<crate::AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let short_links: Collection<ShortLink> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(ShortLink::collection_name());
    let sessions: Collection<Session> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Session::collection_name());

    let link = short_links
        .find_one(doc! { "shortCode": &short_code, "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    let session_id = link
        .session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    Ok(Json(serde_json::json!({
        "sessionId": session.id.unwrap().to_hex(),
        "locationId": session.location_id.to_hex(),
        "description": session.description,
        "expiresAt": session.expires_at,
        "isActive": session.is_active,
    })))
}
