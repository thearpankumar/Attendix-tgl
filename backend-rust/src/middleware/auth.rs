use axum::{extract::State, http::Request, middleware::Next, response::Response};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::models::Admin;

/// Redis key prefix for blacklisted JWT IDs.
const BLACKLIST_PREFIX: &str = "jwt_blacklist:";

/// Cookie name used to store the admin auth token (HttpOnly).
pub const AUTH_COOKIE_NAME: &str = "admin_token";

/// Sent by the Expo mentor app on every request (see mobile/src/api/client.ts)
/// to tell `login`/`register` to skip `Set-Cookie` entirely, rather than
/// setting it and relying on the app to reliably scrub it client-side.
///
/// That client-side approach (clearing Android's native cookie jar after
/// every response) turned out to be racy in practice — the OkHttp cookie
/// jar's sync timing back to the JS layer wasn't reliable enough to
/// guarantee the very next request went out clean, which is what actually
/// matters for the CSRF check on a POST fired right after login. Not
/// setting the cookie for this client at all removes the race at its
/// source instead of chasing it downstream.
///
/// Spoofing this header can only ever *skip* a cookie being set — it can't
/// grant, widen, or bypass anything else — so trusting it unauthenticated
/// here is safe.
pub const SKIP_AUTH_COOKIE_HEADER: &str = "x-attendix-no-cookie";

/// Cookie path scoped to admin API routes only.
const AUTH_COOKIE_PATH: &str = "/api";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Admin ID (UUID string)
    pub id: String,
    /// Expiry timestamp (Unix seconds)
    pub exp: usize,
    /// Issued-at timestamp (Unix seconds)
    pub iat: usize,
    /// Unique token ID used for blacklisting on logout
    pub jti: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedAdmin {
    pub id: Uuid,
    pub role: String,
}

impl AuthenticatedAdmin {
    /// Every admin account is authenticated equally by `auth_middleware`;
    /// `role` was stored but never checked anywhere, so any admin could hit
    /// the highest-impact, system-wide endpoints (dev-bypass toggle,
    /// security settings). Those handlers call this to require
    /// `crate::constants::ROLE_SUPER_ADMIN` explicitly.
    ///
    /// Session/location/batch deletion are deliberately NOT gated here:
    /// they're already scoped at the query level to "any super-admin, or the
    /// mentor who created it" (mentors never actually qualify for
    /// delete/mutate routes on sessions, which check `created_by`, not the
    /// assignment table) — so the equal-privilege risk this guards against
    /// doesn't apply to them the same way it does to
    /// global config that affects every session.
    pub fn require_role(&self, role: &str) -> Result<()> {
        if self.role != role {
            return Err(AppError::Forbidden(format!(
                "This action requires the '{}' role",
                role
            )));
        }
        Ok(())
    }
}

pub fn generate_token(admin_id: &Uuid, jwt_secret: &str, expires_in: &str) -> Result<String> {
    let expiration = parse_expiry(expires_in)?;
    let now = chrono::Utc::now().timestamp() as usize;

    let claims = Claims {
        id: admin_id.to_string(),
        exp: now + expiration,
        iat: now,
        jti: Uuid::new_v4().to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(AppError::Jwt)
}

/// Builds the `Set-Cookie` header value for the admin auth token.
///
/// - `HttpOnly`: JS cannot read it, mitigating XSS token theft.
/// - `SameSite=Strict`: Browser won't send it cross-site, mitigating CSRF.
/// - `Secure`: Only sent over HTTPS (applied in production).
/// - `Path=/api` (`AUTH_COOKIE_PATH` above): the admin routes that need it
///   span /api/admin, /api/admin/security and /api/config, so this can't be
///   scoped any narrower than /api without missing one of them — it also
///   reaches public student endpoints under /api, which is harmless since
///   they never call auth_middleware. Every router that layers
///   auth_middleware MUST also layer csrf_middleware, since the cookie is
///   auto-attached anywhere under /api.
pub fn make_auth_cookie(token: &str, max_age_secs: usize, is_production: bool) -> String {
    let secure = if is_production { "; Secure" } else { "" };
    format!(
        "{}={}; HttpOnly; SameSite=Strict; Path={}; Max-Age={}{}",
        AUTH_COOKIE_NAME, token, AUTH_COOKIE_PATH, max_age_secs, secure
    )
}

/// Builds a `Set-Cookie` header that immediately clears the auth cookie.
pub fn clear_auth_cookie(is_production: bool) -> String {
    let secure = if is_production { "; Secure" } else { "" };
    format!(
        "{}=; HttpOnly; SameSite=Strict; Path={}; Max-Age=0{}",
        AUTH_COOKIE_NAME, AUTH_COOKIE_PATH, secure
    )
}

pub fn parse_expiry(expires_in: &str) -> Result<usize> {
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
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))
}

/// Checks whether a token's `jti` has been blacklisted (i.e. logged out).
///
/// Returns `false` (allow) when Redis is unavailable, so a Redis outage does
/// not lock every admin out. The trade-off is that a logged-out token remains
/// valid until it naturally expires if Redis is down — acceptable given the
/// short window and the `SameSite=Strict` cookie providing additional protection.
pub async fn is_token_blacklisted(redis: &redis::Client, jti: &str) -> bool {
    use redis::AsyncCommands;
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        tracing::warn!(
            jti = %jti,
            "Redis unavailable for blacklist check; allowing token (fail-open)"
        );
        return false;
    };
    let key = format!("{}{}", BLACKLIST_PREFIX, jti);
    conn.exists::<_, bool>(key).await.unwrap_or(false)
}

/// Adds a token's `jti` to the Redis blacklist with TTL = remaining lifetime.
///
/// The TTL ensures the blacklist never grows unboundedly — entries expire the
/// moment the token would have expired naturally anyway.
pub async fn blacklist_token(redis: &redis::Client, jti: &str, exp: usize) {
    use redis::AsyncCommands;
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        tracing::error!(
            jti = %jti,
            "Redis unavailable — token could NOT be blacklisted. \
             It will remain valid until natural expiry."
        );
        return;
    };
    let now = chrono::Utc::now().timestamp() as usize;
    let ttl = exp.saturating_sub(now);
    if ttl == 0 {
        return; // Already expired — nothing to blacklist
    }
    let key = format!("{}{}", BLACKLIST_PREFIX, jti);
    if let Err(e) = conn.set_ex::<_, _, ()>(key, "revoked", ttl as u64).await {
        tracing::error!(jti = %jti, error = %e, "Failed to write token to blacklist");
    } else {
        tracing::info!(jti = %jti, ttl_secs = ttl, "Token blacklisted successfully");
    }
}

/// Extracts a named cookie value from a raw `Cookie` header string.
fn extract_cookie<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}=", name);
    cookie_header
        .split(';')
        .find_map(|part| part.trim().strip_prefix(prefix.as_str()))
}

pub async fn auth_middleware(
    State(state): State<std::sync::Arc<crate::AppState>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response> {
    // Token resolution order:
    //   1. HttpOnly `admin_token` cookie  (new, preferred path)
    //   2. `Authorization: Bearer <token>` header  (backwards-compat for existing
    //      clients / API callers while the frontend migration is in progress)
    let token = {
        let cookie_token = request
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|header| extract_cookie(header, AUTH_COOKIE_NAME))
            .map(str::to_owned);

        let bearer_token = request
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::to_owned);

        cookie_token
            .or(bearer_token)
            .ok_or_else(|| AppError::Unauthorized("Missing authentication".to_string()))?
    };

    let claims = verify_token(&token, &state.config.jwt_secret)?;

    // Tokens are blacklisted on logout so revocation takes effect immediately.
    if is_token_blacklisted(&state.redis, &claims.jti).await {
        return Err(AppError::Unauthorized(
            "Token has been revoked. Please log in again.".to_string(),
        ));
    }

    let admin_id = Uuid::parse_str(&claims.id)
        .map_err(|_| AppError::Unauthorized("Invalid admin ID in token".to_string()))?;

    let admin = sqlx::query_as::<_, Admin>("SELECT * FROM admins WHERE id = $1")
        .bind(admin_id)
        .fetch_optional(&state.db)
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

    // Deactivation (User Management's "deactivate" toggle) must take effect
    // immediately on the account's very next request — the DB row is the
    // source of truth here, checked on every request, so no Redis
    // coordination with the token blacklist is needed.
    if !admin.is_active {
        return Err(AppError::Unauthorized(
            "Account has been deactivated.".to_string(),
        ));
    }

    // Stash the full claims so logout can extract jti + exp without re-parsing
    request.extensions_mut().insert(claims);
    request.extensions_mut().insert(AuthenticatedAdmin {
        id: admin_id,
        role: admin.role.clone(),
    });

    Ok(next.run(request).await)
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
