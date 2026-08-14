use axum::{
    extract::{ConnectInfo, Json, State},
    http::{header::SET_COOKIE, StatusCode},
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::{
        blacklist_token, clear_auth_cookie, generate_token, make_auth_cookie, parse_expiry,
        validators::{validate_request, AdminLoginRequest, AdminRegisterRequest},
        AuthenticatedAdmin, Claims,
    },
    models::{Admin, AdminLogin, AdminRegistration},
};

#[derive(Debug, Serialize)]
pub struct AdminResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
    #[serde(rename = "collegeName")]
    pub college_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// The token is still included in the body for backwards compatibility
    /// while the admin frontend is migrated off localStorage. Once the
    /// frontend reads only the HttpOnly cookie, this field should be removed.
    pub token: String,
    pub expires_in: String,
    pub admin: AdminResponse,
}

pub async fn register(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<AdminRegistration>,
) -> Result<impl IntoResponse> {
    let validation_req = AdminRegisterRequest {
        username: payload.username.clone(),
        email: payload.email.clone(),
        password: payload.password.clone(),
        admin_secret: payload.admin_secret.clone(),
    };
    validate_request(&validation_req)?;

    // Constant-time: a byte-wise `!=` leaks the correct prefix through timing,
    // and this endpoint is public.
    use subtle::ConstantTimeEq;
    let secret_matches: bool = payload
        .admin_secret
        .as_bytes()
        .ct_eq(state.config.admin_secret.as_bytes())
        .into();
    if !secret_matches {
        return Err(AppError::Unauthorized("Invalid admin secret".to_string()));
    }

    // This endpoint is a one-time bootstrap, not an ongoing registration
    // path: it only ever creates the *first* super_admin. Every admin/mentor
    // account after that is created exclusively through the authenticated,
    // role-gated User Management panel (`POST /api/admin/users`). The
    // advisory lock makes the "does a super_admin already exist" check and
    // the insert atomic, so two concurrent requests can't both slip through
    // and mint two bootstrap accounts.
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('admin_bootstrap'))")
        .execute(&mut *tx)
        .await?;

    let super_admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admins WHERE role = $1)")
            .bind(crate::constants::ROLE_SUPER_ADMIN)
            .fetch_one(&mut *tx)
            .await?;

    if super_admin_exists {
        // Disguised the same way the canary account is: a caller holding a
        // valid ADMIN_SECRET but hitting a closed bootstrap window learns
        // nothing beyond "this route doesn't behave like it exists."
        return Err(AppError::NotFound("Not found".to_string()));
    }

    let existing: Option<Admin> = sqlx::query_as("SELECT * FROM admins WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&mut *tx)
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    let existing_email: Option<Admin> = sqlx::query_as("SELECT * FROM admins WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&mut *tx)
        .await?;
    if existing_email.is_some() {
        return Err(AppError::BadRequest("Email already exists".to_string()));
    }

    let hashed_password = Admin::hash_password(&payload.password)?;

    // This is definitionally the one bootstrap admin — the route becomes
    // permanently unusable (404) the moment this commits, so there is no
    // "second admin via this route" case to still service as ROLE_ADMIN.
    let role = crate::constants::ROLE_SUPER_ADMIN;

    let admin: Admin = sqlx::query_as(
        "INSERT INTO admins (id, username, email, password, role, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, 0, now()) \
         RETURNING *",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&hashed_password)
    .bind(role)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    // Self-registration is gated only by a shared, never-rotated
    // ADMIN_SECRET (see the constant-time check above) — a leak of that
    // secret is otherwise silent. Every new admin account is a
    // security-relevant event and should be visible to whoever watches the
    // logs/alerts, not just discoverable by querying the admins table.
    tracing::error!(
        target: "security",
        event = "admin_registered",
        admin_id = %admin.id,
        username = %admin.username,
        email = %admin.email,
        role = %admin.role,
        "New admin account registered via /api/admin/register"
    );

    if let Err(e) = crate::models::record_audit_event(
        &state.db,
        Some(admin.id),
        "admin_registered",
        serde_json::json!({ "username": admin.username, "email": admin.email, "role": admin.role }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for admin_registered");
    }

    let token = generate_token(
        &admin.id,
        &state.config.jwt_secret,
        &state.config.jwt_expire,
    )?;

    let max_age = parse_expiry(&state.config.jwt_expire)?;
    let cookie = make_auth_cookie(&token, max_age, state.config.is_production());

    let body = Json(LoginResponse {
        token: token.clone(),
        expires_in: state.config.jwt_expire.clone(),
        admin: AdminResponse {
            id: admin.id.to_string(),
            username: admin.username,
            email: admin.email,
            role: admin.role,
            full_name: admin.full_name,
            college_name: admin.college_name,
        },
    });

    let mut response = (StatusCode::CREATED, body).into_response();
    if let Ok(val) = axum::http::HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    Ok(response)
}

pub async fn login(
    State(state): State<Arc<crate::AppState>>,
    // Option<Extension<ConnectInfo<_>>>, not a bare ConnectInfo — see
    // middleware::deny_list_middleware's comment; a strict extractor here
    // would 500 in any test/context that doesn't go through
    // into_make_service_with_connect_info.
    connect_info: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    Json(payload): Json<AdminLogin>,
) -> Result<impl IntoResponse> {
    let client_ip = connect_info
        .map(|Extension(ConnectInfo(addr))| addr.ip().to_string())
        .unwrap_or_else(|| "unknown-peer".to_string());

    // Canary account: intercepted before ever touching the database. Anyone
    // typing this exact username is either the operator poking at their own
    // decoy (rare, harmless) or running credential-stuffing/recon against a
    // guessed "superadmin" account (the overwhelmingly common case) — either
    // way, the response looks identical to a normal wrong-password rejection
    // to anyone watching the wire.
    if payload
        .username
        .eq_ignore_ascii_case(crate::constants::CANARY_ADMIN_USERNAME)
    {
        state
            .deny_list
            .add(&client_ip, "canary admin account login attempt")
            .await;
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    let validation_req = AdminLoginRequest {
        username: payload.username.clone(),
        password: payload.password.clone(),
    };
    validate_request(&validation_req)?;

    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if admin.is_locked() {
        return Err(AppError::Unauthorized(
            "Account is locked. Try again later.".to_string(),
        ));
    }

    if !admin.verify_password(&payload.password)? {
        let sys_config = state.get_system_config().await;
        let max_attempts = sys_config.lockout_config.max_login_attempts as i32;
        let lock_duration = sys_config.lockout_config.lockout_duration_minutes as i64;
        let attempts = admin.failed_login_attempts + 1;
        let lock_until = if attempts >= max_attempts {
            Some(chrono::Utc::now() + chrono::Duration::minutes(lock_duration))
        } else {
            None
        };

        sqlx::query("UPDATE admins SET failed_login_attempts = $1, lock_until = $2 WHERE id = $3")
            .bind(attempts)
            .bind(lock_until)
            .bind(admin.id)
            .execute(&state.db)
            .await?;

        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    if admin.failed_login_attempts > 0 {
        sqlx::query("UPDATE admins SET failed_login_attempts = 0, lock_until = NULL WHERE id = $1")
            .bind(admin.id)
            .execute(&state.db)
            .await?;
    }

    let admin_id = admin.id;

    if admin.should_rehash() {
        let new_hash = Admin::hash_password(&payload.password)?;
        sqlx::query("UPDATE admins SET password = $1 WHERE id = $2")
            .bind(&new_hash)
            .bind(admin_id)
            .execute(&state.db)
            .await?;
    }

    let token = generate_token(
        &admin_id,
        &state.config.jwt_secret,
        &state.config.jwt_expire,
    )?;

    if let Err(e) = crate::models::record_audit_event(
        &state.db,
        Some(admin_id),
        "admin_login",
        serde_json::json!({ "username": admin.username }),
        Some(&client_ip),
    )
    .await
    {
        // The audit log is a detective control, not a gate — a failure to
        // record it must never block a legitimate login.
        tracing::error!(error = %e, "Failed to record audit log entry for admin_login");
    }

    let max_age = parse_expiry(&state.config.jwt_expire)?;
    let cookie = make_auth_cookie(&token, max_age, state.config.is_production());

    let body = Json(LoginResponse {
        token: token.clone(),
        expires_in: state.config.jwt_expire.clone(),
        admin: AdminResponse {
            id: admin_id.to_string(),
            username: admin.username,
            email: admin.email,
            role: admin.role,
            full_name: admin.full_name,
            college_name: admin.college_name,
        },
    });

    let mut response = body.into_response();
    if let Ok(val) = axum::http::HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    Ok(response)
}

/// Logs out the current admin session.
///
/// 1. Blacklists the token's `jti` in Redis (TTL = remaining lifetime) so the
///    token becomes immediately invalid even before its natural expiry.
/// 2. Clears the HttpOnly `admin_token` cookie via `Max-Age=0`.
pub async fn logout(
    State(state): State<Arc<crate::AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse> {
    // Blacklist the jti so no further requests can use this token
    blacklist_token(&state.redis, &claims.jti, claims.exp).await;

    let clear_cookie = clear_auth_cookie(state.config.is_production());
    let body = Json(serde_json::json!({ "message": "Logged out successfully" }));
    let mut response = body.into_response();
    if let Ok(val) = axum::http::HeaderValue::from_str(&clear_cookie) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    Ok(response)
}

pub async fn get_profile(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Admin not found".to_string()))?;

    Ok(Json(AdminResponse {
        id: auth.id.to_string(),
        username: admin.username,
        email: admin.email,
        role: admin.role,
        full_name: admin.full_name,
        college_name: admin.college_name,
    }))
}

/// Body for a logged-in admin/mentor changing their own password.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Self-service password change, usable by both super_admin and admin/mentor
/// accounts. Requires re-entering the current password first — the same
/// re-verification pattern `delete_session` uses for destructive actions —
/// since there is no email-based reset flow to fall back on if this were
/// ever abused via a hijacked session.
pub async fn change_own_password(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse> {
    if payload.new_password.len() < 6 {
        return Err(AppError::BadRequest(
            "New password must be at least 6 characters".to_string(),
        ));
    }

    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Admin not found".to_string()))?;

    let password_valid = admin
        .verify_password(&payload.current_password)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {}", e)))?;
    if !password_valid {
        return Err(AppError::Unauthorized(
            "Current password is incorrect".to_string(),
        ));
    }

    let new_hash = Admin::hash_password(&payload.new_password)?;
    sqlx::query("UPDATE admins SET password = $1 WHERE id = $2")
        .bind(&new_hash)
        .bind(auth.id)
        .execute(&state.db)
        .await?;

    if let Err(e) = crate::models::record_audit_event(
        &state.db,
        Some(auth.id),
        "admin_password_changed",
        serde_json::json!({}),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for admin_password_changed");
    }

    Ok(Json(
        serde_json::json!({ "success": true, "message": "Password updated successfully" }),
    ))
}
