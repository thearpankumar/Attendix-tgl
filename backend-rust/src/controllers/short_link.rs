use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Location, Session, ShortLink},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "clickCount")]
    pub click_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Resolves the short code to use for a new short link: a trimmed, non-empty
/// custom code if the admin provided one, otherwise a freshly generated one.
/// An empty string (sent by the admin UI's "auto-generate" mode) must fall
/// through to generation rather than being used literally.
fn resolve_short_code(provided: Option<String>) -> String {
    provided
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| ShortLink::generate_short_code(10))
}

pub async fn create_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateShortLinkRequest>,
) -> Result<impl IntoResponse> {
    let session_id = payload.session_id.and_then(|id| Uuid::parse_str(&id).ok());

    if let Some(sid) = session_id {
        let _session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
        )
        .bind(sid)
        .bind(&auth.role)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found or unauthorized".to_string()))?;

        let existing_link: Option<ShortLink> =
            sqlx::query_as("SELECT * FROM short_links WHERE session_id = $1 AND is_active = true")
                .bind(sid)
                .fetch_optional(&state.db)
                .await?;
        if existing_link.is_some() {
            return Err(AppError::BadRequest(
                "Session already has an active short link".to_string(),
            ));
        }

        // Note: the original Mongo code also set an untyped `totpEnabled` field
        // on the session document here. That field was never read back anywhere
        // in the codebase (dead write) and has no corresponding column in the
        // Postgres schema, so it's intentionally dropped.
    }

    let short_code = resolve_short_code(payload.short_code);

    let existing: Option<ShortLink> =
        sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
            .bind(&short_code)
            .fetch_optional(&state.db)
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

    let inserted = sqlx::query_as::<_, ShortLink>(
        "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, expires_at, click_count, last_clicked_at, created_at) \
         VALUES ($1, $2, $3, $4, true, $5, 0, NULL, $6) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&short_code)
    .bind(session_id)
    .bind(auth.id)
    .bind(expires_at)
    .bind(Utc::now())
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ShortLinkResponse {
            id: inserted.id.to_string(),
            short_code: inserted.short_code.clone(),
            url: format!("{}/s/{}", state.config.public_base_url, inserted.short_code),
            session_id: inserted.session_id.map(|id| id.to_string()),
            expires_at: inserted.expires_at.map(|d| d.to_rfc3339()),
            is_active: inserted.is_active,
            click_count: inserted.click_count,
            created_at: inserted.created_at.to_rfc3339(),
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
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<ShortLinksQuery>,
) -> Result<impl IntoResponse> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);

    let session_id_filter = query
        .session_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let is_active_filter = query.is_active.as_deref().map(|s| s == "true");

    let links = sqlx::query_as::<_, ShortLink>(
        "SELECT * FROM short_links \
         WHERE ($1 = 'super_admin' OR created_by = $2) \
           AND ($3::uuid IS NULL OR session_id = $3) \
           AND ($4::bool IS NULL OR is_active = $4) \
         ORDER BY created_at DESC \
         OFFSET $5 LIMIT $6",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .bind(session_id_filter)
    .bind(is_active_filter)
    .bind((page - 1) * limit)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let response: Vec<ShortLinkResponse> = links
        .into_iter()
        .map(|link| ShortLinkResponse {
            id: link.id.to_string(),
            short_code: link.short_code.clone(),
            url: format!("{}/s/{}", state.config.public_base_url, link.short_code),
            session_id: link.session_id.map(|id| id.to_string()),
            expires_at: link.expires_at.map(|d| d.to_rfc3339()),
            is_active: link.is_active,
            click_count: link.click_count,
            created_at: link.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(serde_json::json!({ "shortLinks": response })))
}

pub async fn get_short_link_by_code(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let link = sqlx::query_as::<_, ShortLink>(
        "SELECT * FROM short_links WHERE short_code = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(short_code.to_lowercase())
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
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

    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active {
        return Err(AppError::BadRequest(
            "Associated session is not active".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "_id": link.id.to_string(),
        "shortCode": link.short_code,
        "sessionId": session_id.to_string(),
        "isActive": link.is_active,
        "expiresAt": link.expires_at,
        "clickCount": link.click_count,
        "session": {
            "_id": session.id.to_string(),
            "description": session.description,
            "isActive": session.is_active,
            "expiresAt": session.expires_at
        }
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let link = sqlx::query_as::<_, ShortLink>("SELECT * FROM short_links WHERE short_code = $1")
        .bind(short_code.to_lowercase())
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    let session_id = Uuid::parse_str(&payload.session_id)
        .map_err(|_| AppError::BadRequest("Invalid session ID format".to_string()))?;

    // Check if the link is already attached to an active session
    if let Some(current_session_id) = link.session_id {
        if current_session_id != session_id {
            if let Some(current_session) =
                sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
                    .bind(current_session_id)
                    .fetch_optional(&state.db)
                    .await?
            {
                if current_session.is_active {
                    return Err(AppError::BadRequest(
                        "Active session short link cannot be reassigned".to_string(),
                    ));
                }
            }
        }
    }

    let _session = sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found or unauthorized".to_string()))?;

    let existing_link: Option<ShortLink> = sqlx::query_as(
        "SELECT * FROM short_links WHERE session_id = $1 AND is_active = true AND id != $2",
    )
    .bind(session_id)
    .bind(link.id)
    .fetch_optional(&state.db)
    .await?;

    if let Some(existing) = existing_link {
        sqlx::query("UPDATE short_links SET session_id = NULL, is_active = false WHERE id = $1")
            .bind(existing.id)
            .execute(&state.db)
            .await?;
    }

    sqlx::query("UPDATE short_links SET session_id = $1, is_active = true WHERE id = $2")
        .bind(session_id)
        .bind(link.id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Short link attached successfully",
        "shortCode": short_code,
        "sessionId": session_id.to_string()
    })))
}

pub async fn detach_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let link = sqlx::query_as::<_, ShortLink>(
        "SELECT * FROM short_links WHERE short_code = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(short_code.to_lowercase())
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    sqlx::query("UPDATE short_links SET session_id = NULL, is_active = false WHERE id = $1")
        .bind(link.id)
        .execute(&state.db)
        .await?;

    Ok(Json(
        serde_json::json!({ "message": "Short link detached successfully" }),
    ))
}

pub async fn delete_short_link(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let result = sqlx::query(
        "DELETE FROM short_links WHERE short_code = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(short_code.to_lowercase())
    .bind(&auth.role)
    .bind(auth.id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
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
    let sessions = sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE is_active = true AND ($1 = 'super_admin' OR created_by = $2) ORDER BY created_at DESC",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    let results: Vec<serde_json::Value> = sessions
        .into_iter()
        .map(|session| {
            serde_json::json!({
                "_id": session.id.to_string(),
                "description": session.description,
                "expiresAt": session.expires_at,
                "createdAt": session.created_at
            })
        })
        .collect();

    Ok(Json(results))
}

pub async fn resolve_short_link(
    State(state): State<Arc<crate::AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let link = sqlx::query_as::<_, ShortLink>(
        "SELECT * FROM short_links WHERE short_code = $1 AND is_active = true",
    )
    .bind(&short_code)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    if link.is_expired() {
        return Err(AppError::NotFound("Short link has expired".to_string()));
    }

    sqlx::query("UPDATE short_links SET click_count = click_count + 1, last_clicked_at = now() WHERE id = $1")
        .bind(link.id)
        .execute(&state.db)
        .await?;

    let redirect_url = format!("{}/attend/{}", state.config.public_base_url, short_code);

    Ok(([("Location", redirect_url)], StatusCode::FOUND))
}

pub async fn get_short_link_session(
    State(state): State<Arc<crate::AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let link = sqlx::query_as::<_, ShortLink>(
        "SELECT * FROM short_links WHERE short_code = $1 AND is_active = true",
    )
    .bind(&short_code)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Short link not found".to_string()))?;

    let session_id = link
        .session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let location = sqlx::query_as::<_, Location>("SELECT * FROM locations WHERE id = $1")
        .bind(session.location_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let sys_config = state.get_system_config().await;
    let dev_bypass_enabled = sys_config.dev_bypass_enabled
        || std::env::var("DEV_BYPASS_ALL").unwrap_or_default() == "true";

    Ok(Json(serde_json::json!({
        "valid": true,
        "session": {
            "sessionId": session.id.to_string(),
            "locationId": session.location_id.map(|id| id.to_string()),
            "locationName": location.name,
            "description": session.description,
            "expiresAt": session.expires_at,
            "isActive": session.is_active,
        },
        "devBypassEnabled": dev_bypass_enabled,
    })))
}

#[cfg(test)]
mod create_short_link_tests {
    use super::*;

    /// Regression test: CreateShortLinkRequest must deserialize the
    /// camelCase field names ShortLinks.tsx actually sends
    /// (`{shortCode, sessionId}`), not the snake_case keys they'd need
    /// without `#[serde(rename_all = "camelCase")]`.
    #[test]
    fn deserializes_camel_case_fields() {
        let payload: CreateShortLinkRequest = serde_json::from_value(serde_json::json!({
            "shortCode": "custom123",
            "sessionId": "abc123",
        }))
        .unwrap();

        assert_eq!(payload.short_code.as_deref(), Some("custom123"));
        assert_eq!(payload.session_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn resolve_short_code_uses_provided_custom_code() {
        let resolved = resolve_short_code(Some("custom123".to_string()));
        assert_eq!(resolved, "custom123");
    }

    /// Regression test: the admin UI's "auto-generate" mode sends
    /// `shortCode: ""`. Once the camelCase deserialization fix made this
    /// value actually reach the handler as `Some("")`, using it literally
    /// (instead of falling back to generation) would create a short link
    /// with an empty code.
    #[test]
    fn resolve_short_code_falls_back_to_generated_code_for_empty_string() {
        let resolved = resolve_short_code(Some("".to_string()));
        assert_eq!(resolved.len(), 10);
        assert_ne!(resolved, "");

        let resolved_whitespace = resolve_short_code(Some("   ".to_string()));
        assert_eq!(resolved_whitespace.len(), 10);
    }

    #[test]
    fn resolve_short_code_falls_back_to_generated_code_for_none() {
        let resolved = resolve_short_code(None);
        assert_eq!(resolved.len(), 10);
    }
}
