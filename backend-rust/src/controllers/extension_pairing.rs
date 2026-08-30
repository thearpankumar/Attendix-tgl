//! Phase 3: browser-extension device pairing. Reuses the existing WebAuthn
//! ceremony (see `controllers::public_webauthn::{resolve_authentication_subject,
//! verify_passkey_assertion}`) rather than a parallel auth system — a phone
//! scans the extension's QR, authenticates with its enrolled passkey exactly
//! like an attendance check-in would, and that ceremony's success is what
//! completes the pairing.
//!
//! Deliberately a SEPARATE endpoint from `finish_authentication` (attendance
//! submission), not that endpoint extended with a flag: `finish_authentication`
//! refuses outright once attendance is already submitted for (session,
//! roll_number), which would silently block exactly the "student changes
//! device mid-session" case (plan Part E step 8) this flow exists to support
//! — a student who already checked in must still be able to re-pair a
//! replacement laptop without re-submitting attendance.

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    controllers::public_webauthn::{
        load_active_short_link_and_session, resolve_authentication_subject,
        verify_passkey_assertion,
    },
    error::{AppError, Result},
    middleware::blacklist_token,
    models::{generate_telemetry_token, verify_telemetry_token, ExtensionPairingRequest, ShortLink},
    AppState,
};

const PAIRING_CODE_LENGTH: usize = 8;
const PAIRING_TTL_MINUTES: i64 = 5;

async fn find_pairing_request(
    pool: &sqlx::PgPool,
    pairing_code: &str,
) -> Result<ExtensionPairingRequest> {
    sqlx::query_as("SELECT * FROM extension_pairing_requests WHERE pairing_code = $1")
        .bind(pairing_code)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Pairing request not found".to_string()))
}

// =================== Start ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPairingRequest {
    pub extension_instance_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPairingResponse {
    pub pairing_code: String,
    /// A plain string, not an image — the extension renders this as a QR
    /// code itself, client-side (plan Part E step 3). Must be a fully
    /// qualified URL (not just a path) — a phone's camera app needs a real
    /// scheme+host to treat this as an openable link, and it points at the
    /// student-frontend SPA's own pairing route (`/attend/pair/...`, handled
    /// by `frontend/student`'s `ExtensionPair` page), not at the API route
    /// this handler lives under.
    pub pairing_url: String,
}

async fn generate_unique_pairing_code(pool: &sqlx::PgPool) -> Result<String> {
    let mut attempts = 0;
    loop {
        let candidate = ShortLink::generate_short_code(PAIRING_CODE_LENGTH);
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM extension_pairing_requests WHERE pairing_code = $1")
                .bind(&candidate)
                .fetch_optional(pool)
                .await?;
        if existing.is_none() {
            return Ok(candidate);
        }
        attempts += 1;
        if attempts > 10 {
            return Err(AppError::Internal(
                "Failed to generate unique pairing code".to_string(),
            ));
        }
    }
}

pub async fn start_pairing(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<StartPairingRequest>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let pairing_code = generate_unique_pairing_code(&state.db).await?;

    sqlx::query(
        "INSERT INTO extension_pairing_requests \
         (id, session_id, extension_instance_id, pairing_code, status, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, 'pending', now(), $5)",
    )
    .bind(Uuid::new_v4())
    .bind(session.id)
    .bind(payload.extension_instance_id)
    .bind(&pairing_code)
    .bind(Utc::now() + Duration::minutes(PAIRING_TTL_MINUTES))
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(StartPairingResponse {
            pairing_url: format!(
                "{}/attend/pair/{short_code}/{pairing_code}",
                state.config.public_base_url
            ),
            pairing_code,
        }),
    ))
}

// =================== Status ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll_number: Option<String>,
}

pub async fn pairing_status(
    State(state): State<Arc<AppState>>,
    Path((_short_code, pairing_code)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let request = find_pairing_request(&state.db, &pairing_code).await?;

    let status = if request.status == "pending" && request.is_expired() {
        "expired"
    } else {
        request.status.as_str()
    };

    Ok(Json(PairingStatusResponse {
        status: status.to_string(),
        roll_number: if status == "completed" {
            request.roll_number
        } else {
            None
        },
    }))
}

// =================== Finish authentication (phone side) ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPairingAuthenticationRequest {
    pub pairing_code: String,
    pub credential: webauthn_rs::prelude::PublicKeyCredential,
    /// The `challengeId` `POST /{shortCode}/webauthn/authenticate/start`
    /// returned. Required — correlates this finish call with its own
    /// pending challenge row instead of guessing the most recently created
    /// one. See `public_webauthn::take_pending_challenge`'s doc comment.
    pub challenge_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPairingAuthenticationResponse {
    pub message: String,
}

/// The phone-side half of pairing: completes the same WebAuthn ceremony the
/// extension's QR pointed the phone at (started via the existing
/// `POST /{shortCode}/webauthn/authenticate/start`), then marks the pairing
/// request completed. No GPS, no photo, no attendance row — this is
/// identity+device pairing, not attendance.
pub async fn finish_pairing_authentication(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<FinishPairingAuthenticationRequest>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let pairing_request = find_pairing_request(&state.db, &payload.pairing_code).await?;
    if pairing_request.session_id != session.id {
        return Err(AppError::BadRequest(
            "Pairing code does not belong to this session".to_string(),
        ));
    }
    if pairing_request.status != "pending" {
        return Err(AppError::BadRequest(
            "Pairing request is no longer pending".to_string(),
        ));
    }
    if pairing_request.is_expired() {
        return Err(AppError::BadRequest(
            "Pairing request has expired".to_string(),
        ));
    }

    let (roll_upper, stored_challenge) = resolve_authentication_subject(
        &state,
        &short_code,
        payload.challenge_id,
        &payload.credential,
    )
    .await?;

    let (stored_credential, _counter) = verify_passkey_assertion(
        &state,
        &roll_upper,
        &stored_challenge,
        &payload.credential,
        session.id,
    )
    .await?;

    // `status = 'pending'` in the WHERE, not just the earlier check above,
    // closes the race between two concurrent finishes for the same code.
    let updated = sqlx::query(
        "UPDATE extension_pairing_requests \
         SET status = 'completed', roll_number = $1, webauthn_credential_id = $2, completed_at = now() \
         WHERE id = $3 AND status = 'pending'",
    )
    .bind(&roll_upper)
    .bind(stored_credential.id)
    .bind(pairing_request.id)
    .execute(&state.db)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "Pairing request was completed or expired concurrently".to_string(),
        ));
    }

    Ok(Json(FinishPairingAuthenticationResponse {
        message: "Device paired successfully".to_string(),
    }))
}

// =================== Finish (extension side) ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPairingRequest {
    pub pairing_code: String,
    pub device_fingerprint_hash: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPairingResponse {
    pub locked: bool,
    pub roll_number: String,
    /// Signed telemetry bearer token for `POST .../extension/events` (see
    /// `models::telemetry_token`). Extension stores this and sends it as
    /// `Authorization: Bearer <token>` — see `extension/lib/api.ts`.
    pub telemetry_token: String,
    pub telemetry_token_expires_at: chrono::DateTime<Utc>,
}

/// The extension-side half of pairing: called once the extension polls
/// `.../pair/status` and sees `completed`. Writes the actual device lock —
/// superseding (never silently overwriting) any prior active lock for this
/// (session, roll_number), which is what makes a mid-session device change
/// (plan Part E step 8) an audit trail instead of a silent swap.
pub async fn finish_pairing(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<FinishPairingRequest>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let pairing_request = find_pairing_request(&state.db, &payload.pairing_code).await?;
    if pairing_request.session_id != session.id {
        return Err(AppError::BadRequest(
            "Pairing code does not belong to this session".to_string(),
        ));
    }
    if pairing_request.status != "completed" {
        return Err(AppError::BadRequest(
            "Pairing has not completed authentication yet".to_string(),
        ));
    }
    if pairing_request.is_expired() {
        return Err(AppError::BadRequest(
            "Pairing request has expired".to_string(),
        ));
    }
    let roll_number = pairing_request.roll_number.clone().ok_or_else(|| {
        AppError::Internal("Completed pairing request has no roll number".to_string())
    })?;

    // Idempotency: `extension_pairing_requests.status` stays 'completed'
    // forever once set (nothing marks it "consumed"), so a retried call with
    // the same pairing_code — e.g. the extension's popup retrying after a
    // network error on its first attempt — would otherwise supersede the
    // lock it JUST created and insert a fresh duplicate, producing a
    // spurious "device switched" entry in the audit trail for a laptop that
    // never actually changed. If the current active lock already belongs to
    // this same extension install, this call has already succeeded before —
    // report success without writing anything new.
    let already_locked_to_this_device: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM session_device_locks \
         WHERE session_id = $1 AND roll_number = $2 AND status = 'active' \
         AND extension_instance_id = $3",
    )
    .bind(session.id)
    .bind(&roll_number)
    .bind(pairing_request.extension_instance_id)
    .fetch_optional(&state.db)
    .await?;

    if let Some(existing_lock_id) = already_locked_to_this_device {
        // The extension never received a response on its first attempt (that's
        // the only way this branch is reached), so no previously-issued token
        // was ever used — issue a fresh one and update the still-active lock
        // in place; nothing to blacklist.
        let issued = generate_telemetry_token(
            session.id,
            &roll_number,
            pairing_request.extension_instance_id,
            existing_lock_id,
            &state.config.jwt_secret,
        )?;
        sqlx::query(
            "UPDATE session_device_locks SET telemetry_token_jti = $1, telemetry_token_expires_at = $2 \
             WHERE id = $3",
        )
        .bind(&issued.jti)
        .bind(issued.expires_at)
        .bind(existing_lock_id)
        .execute(&state.db)
        .await?;

        return Ok((
            StatusCode::CREATED,
            Json(FinishPairingResponse {
                locked: true,
                roll_number,
                telemetry_token: issued.token,
                telemetry_token_expires_at: issued.expires_at,
            }),
        ));
    }

    let mut tx = state.db.begin().await?;

    // Upsert the durable "which physical install is this" anchor.
    sqlx::query(
        "INSERT INTO extension_installations \
         (id, roll_number, extension_instance_id, first_seen_at, last_seen_at, browser, os, is_active) \
         VALUES ($1, $2, $3, now(), now(), $4, $5, true) \
         ON CONFLICT (extension_instance_id) DO UPDATE \
         SET last_seen_at = now(), browser = EXCLUDED.browser, os = EXCLUDED.os, is_active = true",
    )
    .bind(Uuid::new_v4())
    .bind(&roll_number)
    .bind(pairing_request.extension_instance_id)
    .bind(&payload.browser)
    .bind(&payload.os)
    .execute(&mut *tx)
    .await?;

    // Never silently overwrite a prior active lock — supersede it instead.
    // Three steps, in this exact order, because `superseded_by` is an FK
    // (checked immediately, not deferred) and the unique partial index on
    // (session_id, roll_number) WHERE status = 'active' rejects a second
    // active row while the first still exists: (1) demote the old row to
    // 'superseded' WITHOUT yet pointing `superseded_by` anywhere, freeing
    // the active slot and satisfying the unique index; (2) insert the new
    // active row, now that nothing else holds that slot; (3) point the old
    // row's `superseded_by` at the new row, which now exists so the FK is
    // satisfiable. Steps 1+2 reversed would violate the unique index; the FK
    // makes setting `superseded_by` in step 1 impossible before step 2 runs.
    let new_lock_id = Uuid::new_v4();
    // Capture the just-superseded row's telemetry token so it can be
    // blacklisted after commit — rotation-on-supersede, mirroring how the
    // admin JWT is revoked on logout (middleware::auth::blacklist_token).
    let superseded_token: Option<(Option<String>, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        "UPDATE session_device_locks \
         SET status = 'superseded', released_at = now() \
         WHERE session_id = $1 AND roll_number = $2 AND status = 'active' \
         RETURNING telemetry_token_jti, telemetry_token_expires_at",
    )
    .bind(session.id)
    .bind(&roll_number)
    .fetch_optional(&mut *tx)
    .await?;

    let issued = generate_telemetry_token(
        session.id,
        &roll_number,
        pairing_request.extension_instance_id,
        new_lock_id,
        &state.config.jwt_secret,
    )?;

    sqlx::query(
        "INSERT INTO session_device_locks \
         (id, session_id, roll_number, extension_instance_id, webauthn_credential_id, device_fingerprint_hash, locked_at, status, telemetry_token_jti, telemetry_token_expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, now(), 'active', $7, $8)",
    )
    .bind(new_lock_id)
    .bind(session.id)
    .bind(&roll_number)
    .bind(pairing_request.extension_instance_id)
    .bind(pairing_request.webauthn_credential_id)
    .bind(&payload.device_fingerprint_hash)
    .bind(&issued.jti)
    .bind(issued.expires_at)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE session_device_locks SET superseded_by = $1 \
         WHERE session_id = $2 AND roll_number = $3 AND status = 'superseded' AND superseded_by IS NULL",
    )
    .bind(new_lock_id)
    .bind(session.id)
    .bind(&roll_number)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Best-effort: a blacklist failure here (e.g. Redis outage) leaves the
    // old token valid until its own natural expiry rather than blocking the
    // new pairing — matches blacklist_token's own fail-open behavior.
    if let Some((Some(old_jti), Some(old_expires_at))) = superseded_token {
        blacklist_token(&state.redis, &old_jti, old_expires_at.timestamp() as usize).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(FinishPairingResponse {
            locked: true,
            roll_number,
            telemetry_token: issued.token,
            telemetry_token_expires_at: issued.expires_at,
        }),
    ))
}

// =================== Telemetry token refresh ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTelemetryTokenResponse {
    pub telemetry_token: String,
    pub telemetry_token_expires_at: chrono::DateTime<Utc>,
}

/// `POST /{shortCode}/extension/telemetry/token/refresh` — reissues a token
/// for a lock that's still active, without a full WebAuthn re-ceremony. The
/// extension calls this proactively a few minutes before its current token
/// expires (see `extension/entrypoints/background.ts`), so a legitimately
/// paired device never has to re-pair mid-session just to keep telemetry
/// flowing.
///
/// The presented token must still verify cryptographically (so an already-
/// fully-expired token can't be refreshed — that's a deliberate ceiling: a
/// device that's been offline past its token's lifetime must re-pair via QR,
/// same as the pairing flow's own 5-minute pairing-code TTL is not
/// extendable). Not blacklisted on refresh (unlike the supersede path in
/// `finish_pairing`) — refreshing is a routine, non-security-relevant event,
/// and the lock row's `telemetry_token_jti` column being overwritten already
/// makes the previous token unusable via `ingest_telemetry`'s lock-column
/// check, without needing a Redis write on every refresh cycle.
pub async fn refresh_telemetry_token(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing telemetry token".to_string()))?;

    let claims = verify_telemetry_token(token, &state.config.jwt_secret)?;
    if claims.session_id != session.id {
        return Err(AppError::Unauthorized(
            "Telemetry token does not belong to this session".to_string(),
        ));
    }

    let still_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM session_device_locks \
         WHERE id = $1 AND status = 'active' AND extension_instance_id = $2)",
    )
    .bind(claims.lock_id)
    .bind(claims.extension_instance_id)
    .fetch_one(&state.db)
    .await?;

    if !still_active {
        return Err(AppError::Forbidden(
            "This device's pairing is no longer active — re-pair to continue.".to_string(),
        ));
    }

    let issued = generate_telemetry_token(
        session.id,
        &claims.roll_number,
        claims.extension_instance_id,
        claims.lock_id,
        &state.config.jwt_secret,
    )?;

    sqlx::query(
        "UPDATE session_device_locks SET telemetry_token_jti = $1, telemetry_token_expires_at = $2 \
         WHERE id = $3",
    )
    .bind(&issued.jti)
    .bind(issued.expires_at)
    .bind(claims.lock_id)
    .execute(&state.db)
    .await?;

    Ok(Json(RefreshTelemetryTokenResponse {
        telemetry_token: issued.token,
        telemetry_token_expires_at: issued.expires_at,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UninstallQuery {
    extension_instance_id: Uuid,
}

/// `GET /extension/uninstall?extensionInstanceId=...` — the target of
/// `chrome.runtime.setUninstallURL`, which the extension (re)registers each
/// time it completes a pairing (background.ts's `PAIRING_COMPLETE` handler),
/// since that's the only point it has both a live `apiBase` and a confirmed
/// `extension_instance_id` to build the URL from. Not scoped under a
/// `{shortCode}`, unlike the rest of this file's routes — the identity this
/// marks inactive is the durable device install
/// (`extension_installations.extension_instance_id` is globally unique, not
/// per-session), and the short link that was active when the URL was last
/// registered may well have expired by the time uninstall actually happens.
/// Best-effort by nature (a GET the browser fires on its own, not something
/// the extension can retry) — always returns 200 so it never surfaces an
/// error page to the user mid-uninstall, whether or not a row was found.
pub async fn uninstall_extension(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<UninstallQuery>,
) -> Result<impl IntoResponse> {
    sqlx::query(
        "UPDATE extension_installations SET is_active = false, last_seen_at = now() \
         WHERE extension_instance_id = $1",
    )
    .bind(query.extension_instance_id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::OK)
}
