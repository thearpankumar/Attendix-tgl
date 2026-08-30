//! Signed, short-lived bearer token for the browser extension's ongoing
//! telemetry ingestion (`controllers::telemetry::ingest_telemetry`).
//!
//! Before this, ongoing telemetry trusted a plaintext, client-supplied
//! `extensionInstanceId` matched against the active `session_device_locks`
//! row — no signature, no expiry, no rotation when a new device superseded
//! an old lock. Mirrors the existing admin-JWT pattern
//! (`middleware::auth::{generate_token, verify_token}`) rather than
//! inventing a second token scheme: same `jsonwebtoken` crate, same
//! `state.config.jwt_secret`, same Redis `jti`-blacklist revocation
//! (`middleware::auth::{blacklist_token, is_token_blacklisted}`) reused
//! verbatim for rotation-on-supersede.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Short expiry by design — the extension proactively refreshes well before
/// this via `POST /{shortCode}/extension/telemetry/token/refresh`, so a
/// legitimate paired device never hits a hard 403 mid-session; a stolen
/// token has a narrow window either way.
pub const TELEMETRY_TOKEN_LIFETIME_MINUTES: i64 = 30;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelemetryClaims {
    pub session_id: Uuid,
    pub roll_number: String,
    pub extension_instance_id: Uuid,
    /// The `session_device_locks.id` this token was issued for — checked
    /// against the *currently active* lock's id on every use, not just
    /// `extension_instance_id`, so a token also proves *when* pairing
    /// happened, not only *which device*.
    pub lock_id: Uuid,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,
}

pub struct IssuedTelemetryToken {
    pub token: String,
    pub jti: String,
    pub expires_at: DateTime<Utc>,
}

pub fn generate_telemetry_token(
    session_id: Uuid,
    roll_number: &str,
    extension_instance_id: Uuid,
    lock_id: Uuid,
    jwt_secret: &str,
) -> Result<IssuedTelemetryToken> {
    let now = Utc::now();
    let expires_at = now + Duration::minutes(TELEMETRY_TOKEN_LIFETIME_MINUTES);
    let jti = Uuid::new_v4().to_string();

    let claims = TelemetryClaims {
        session_id,
        roll_number: roll_number.to_string(),
        extension_instance_id,
        lock_id,
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
        jti: jti.clone(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(AppError::Jwt)?;

    Ok(IssuedTelemetryToken {
        token,
        jti,
        expires_at,
    })
}

pub fn verify_telemetry_token(token: &str, jwt_secret: &str) -> Result<TelemetryClaims> {
    decode::<TelemetryClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized("Invalid or expired telemetry token".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-telemetry-secret-not-used-in-production";

    #[test]
    fn round_trips_claims_through_generate_and_verify() {
        let session_id = Uuid::new_v4();
        let extension_instance_id = Uuid::new_v4();
        let lock_id = Uuid::new_v4();

        let issued =
            generate_telemetry_token(session_id, "cs101", extension_instance_id, lock_id, SECRET)
                .unwrap();

        let claims = verify_telemetry_token(&issued.token, SECRET).unwrap();
        assert_eq!(claims.session_id, session_id);
        assert_eq!(claims.roll_number, "cs101");
        assert_eq!(claims.extension_instance_id, extension_instance_id);
        assert_eq!(claims.lock_id, lock_id);
        assert_eq!(claims.jti, issued.jti);
    }

    #[test]
    fn rejects_a_token_signed_with_a_different_secret() {
        let issued = generate_telemetry_token(
            Uuid::new_v4(),
            "cs101",
            Uuid::new_v4(),
            Uuid::new_v4(),
            SECRET,
        )
        .unwrap();

        assert!(verify_telemetry_token(&issued.token, "a-completely-different-secret").is_err());
    }

    #[test]
    fn rejects_a_tampered_token() {
        let issued = generate_telemetry_token(
            Uuid::new_v4(),
            "cs101",
            Uuid::new_v4(),
            Uuid::new_v4(),
            SECRET,
        )
        .unwrap();

        let mut tampered = issued.token.clone();
        tampered.push('x');
        assert!(verify_telemetry_token(&tampered, SECRET).is_err());
    }

    #[test]
    fn expiry_is_set_to_the_configured_lifetime() {
        let before = Utc::now();
        let issued = generate_telemetry_token(
            Uuid::new_v4(),
            "cs101",
            Uuid::new_v4(),
            Uuid::new_v4(),
            SECRET,
        )
        .unwrap();
        let after = Utc::now();

        let expected_min = before + Duration::minutes(TELEMETRY_TOKEN_LIFETIME_MINUTES);
        let expected_max = after + Duration::minutes(TELEMETRY_TOKEN_LIFETIME_MINUTES);
        assert!(issued.expires_at >= expected_min && issued.expires_at <= expected_max);
    }
}
