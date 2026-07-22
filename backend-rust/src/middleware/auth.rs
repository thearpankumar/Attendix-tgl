use axum::{extract::State, http::Request, middleware::Next, response::Response};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use mongodb::{bson::doc, bson::oid::ObjectId, Collection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::models::Admin;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedAdmin {
    pub id: ObjectId,
    pub role: String,
}

pub fn generate_token(admin_id: &ObjectId, jwt_secret: &str, expires_in: &str) -> Result<String> {
    let expiration = parse_expiry(expires_in)?;
    let now = chrono::Utc::now().timestamp() as usize;

    let claims = Claims {
        sub: admin_id.to_hex(),
        exp: now + expiration,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(AppError::Jwt)
}

fn parse_expiry(expires_in: &str) -> Result<usize> {
    let num: usize = expires_in
        .trim_end_matches(|c: char| !c.is_numeric())
        .parse()
        .unwrap_or(7);

    let unit = expires_in.trim_start_matches(|c: char| c.is_numeric());

    Ok(match unit {
        "d" | "day" | "days" => num * 24 * 60 * 60,
        "h" | "hour" | "hours" => num * 60 * 60,
        "m" | "min" | "minute" | "minutes" => num * 60,
        "s" | "sec" | "second" | "seconds" => num,
        _ => num * 24 * 60 * 60,
    })
}

pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))
}

pub async fn auth_middleware(
    State(state): State<std::sync::Arc<crate::AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response> {
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let token = auth_header
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

    let claims = verify_token(token, &state.config.jwt_secret)?;

    let admin_id = ObjectId::parse_str(&claims.sub)
        .map_err(|e| AppError::Unauthorized(format!("Invalid admin ID: {}", e)))?;

    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let collection: Collection<Admin> = state
        .db
        .database(db_name)
        .collection(Admin::collection_name());

    let admin = collection
        .find_one(doc! { "_id": admin_id })
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    if admin.is_locked() {
        let lock_msg = admin
            .lock_until
            .map(|t| format!("Try again after {}", t.format("%H:%M")))
            .unwrap_or_else(|| "Try again later".to_string());
        return Err(AppError::Unauthorized(format!(
            "Admin account is locked. {}",
            lock_msg
        )));
    }

    request.extensions_mut().insert(AuthenticatedAdmin {
        id: admin_id,
        role: admin.role.clone(),
    });

    Ok(next.run(request).await)
}

/// Middleware to require specific role for endpoint access
pub fn require_role(
    required_role: &'static str,
) -> impl Fn(
    Request<axum::body::Body>,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>> + Send + 'static>> {
    move |request: Request<axum::body::Body>, next: Next| {
        Box::pin(async move {
            let auth = request
                .extensions()
                .get::<AuthenticatedAdmin>()
                .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

            if auth.role != required_role && auth.role != "superadmin" {
                return Err(AppError::Forbidden(format!(
                    "Requires {} role",
                    required_role
                )));
            }

            Ok(next.run(request).await)
        })
    }
}

impl<S> axum::extract::FromRequestParts<S> for AuthenticatedAdmin
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self> {
        parts
            .extensions
            .get::<AuthenticatedAdmin>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))
    }
}
