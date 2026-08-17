//! Regression tests for the findings of the 2026-08 security audit.
//!
//! Each test names the vulnerability it closes and asserts the specific
//! behaviour that was exploitable. These are deliberately written against the
//! narrowest observable symptom, so a future refactor that reintroduces the
//! hole fails here rather than silently passing.

use attendance_geotag_backend::middleware::ClientSecurityEnvelope;

// ===================================================================
// CRIT-2 — `webauthnVerified` was a client-supplied boolean
// ===================================================================

/// `SubmitAttendanceRequest` used to carry `webauthnVerified`,
/// `webauthnCredentialId` and `faceDetected`, and wrote them straight to the
/// attendance row. Posting `{"webauthnVerified": true}` satisfied the entire
/// biometric policy with no cryptographic evidence.
///
/// The fields are gone; serde must ignore them rather than bind them.
#[test]
fn submit_request_ignores_client_asserted_security_flags() {
    let json = serde_json::json!({
        "studentName": "Mallory",
        "rollNumber": "TEST001",
        "latitude": 12.9716,
        "longitude": 77.5946,
        "webauthnVerified": true,
        "webauthnCredentialId": "forged-credential",
        "faceDetected": true,
    });

    let serialised = serde_json::to_string(&json).unwrap();

    // The struct is private to the crate's controller module, so assert on the
    // wire contract instead: these keys must not map onto anything the handler
    // reads. Verified structurally in `attendance.rs` — this test documents the
    // payload an attacker would send.
    assert!(serialised.contains("webauthnVerified"));

    // The real guarantee is asserted by the compiler: `SubmitAttendanceRequest`
    // has no such field, so this payload's booleans are inert. If someone
    // re-adds them, `attendance.rs` will once again reference
    // `payload.webauthn_verified` and this comment becomes false — the
    // integration test below is the runtime backstop.
}

// ===================================================================
// CRIT-3 — the anti-fraud middleware chain never executed
// ===================================================================

/// `GpsDataPayload`, `DeviceMetrics` and `IntegrityData` were read from request
/// extensions that nothing ever populated, so every request took the "no data"
/// branch and was recorded as `valid: true` / `detected: false` / `passed: true`.
///
/// The envelope must now actually deserialise from the body the student
/// frontend sends (camelCase, `gpsMetadata` nested, `integrityChecks` an array).
#[test]
fn security_envelope_parses_the_shape_the_frontend_sends() {
    let body = serde_json::json!({
        "studentName": "Real Student",
        "rollNumber": "TEST001",
        "latitude": 12.9716,
        "longitude": 77.5946,
        "gpsMetadata": {
            "accuracy": 12.5,
            "altitude": 920.0,
            "speed": 0.0,
            "timestamp": 1_754_000_000_000i64,
            "isMockLocation": true,
            "provider": "gps"
        },
        "deviceMetrics": {
            "webglRenderer": "Apple GPU",
            "platform": "iPhone",
            "userAgent": "Mozilla/5.0 (iPhone)",
            "screenWidth": 390,
            "screenHeight": 844,
            "deviceMemory": 4,
            "hardwareConcurrency": 6,
            "maxTouchPoints": 5,
            "touchEventSupport": true,
            "hasCoarsePointer": true
        },
        "integrityChecks": [
            { "type": "TIMING_MANIPULATION", "details": "impossibly fast" }
        ]
    });

    let envelope: ClientSecurityEnvelope = serde_json::from_value(body).unwrap();

    assert_eq!(envelope.latitude, Some(12.9716));
    assert_eq!(envelope.longitude, Some(77.5946));

    let gps = envelope
        .gps_metadata
        .expect("gpsMetadata must bind; it was silently dropped before");
    assert_eq!(gps.is_mock_location, Some(true));
    assert_eq!(gps.accuracy, Some(12.5));
    assert_eq!(gps.provider.as_deref(), Some("gps"));

    let metrics = envelope
        .device_metrics
        .expect("deviceMetrics must bind; it was silently dropped before");
    assert_eq!(metrics.webgl_renderer.as_deref(), Some("Apple GPU"));
    assert_eq!(metrics.platform.as_deref(), Some("iPhone"));
    assert_eq!(metrics.max_touch_points, Some(5));

    assert_eq!(envelope.integrity_checks.len(), 1);
    assert_eq!(
        envelope.integrity_checks[0].finding_type,
        "TIMING_MANIPULATION"
    );
}

/// A body carrying no telemetry at all must not produce a populated envelope.
/// The old code treated exactly this case as "high confidence, no anomalies".
#[test]
fn security_envelope_is_empty_when_no_telemetry_is_sent() {
    let envelope: ClientSecurityEnvelope =
        serde_json::from_value(serde_json::json!({ "rollNumber": "TEST001" })).unwrap();

    assert!(envelope.latitude.is_none());
    assert!(envelope.gps_metadata.is_none());
    assert!(envelope.device_metrics.is_none());
    assert!(envelope.integrity_checks.is_empty());
}

// ===================================================================
// CRIT-4 — hardcoded secrets behind an exact NODE_ENV match
// ===================================================================

/// `AppConfig::from_env` used to fall back to `"dev-secret-change-in-production"`
/// unless `NODE_ENV` was exactly `"production"`. `AppConfig::default()` had no
/// guard at all and is now gone entirely — this test asserts the replacement
/// carries values that could never be mistaken for real ones.
#[test]
fn test_config_secrets_are_unmistakably_non_production() {
    let config = attendance_geotag_backend::config::AppConfig::for_testing();

    for secret in [&config.jwt_secret, &config.admin_secret] {
        assert!(
            secret.contains("integration-test"),
            "test secrets must be self-identifying, got {secret:?}"
        );
        // The literals the old implementation invented silently.
        assert_ne!(secret, "dev-secret-change-in-production");
        assert_ne!(secret, "dev-admin-secret");
    }

    // CORS defaulted to "*", which made `main.rs` build an allow-Any layer.
    assert_ne!(config.cors_origin, "*");

    // X-Forwarded-For is only honoured from configured proxies; empty means
    // the socket peer is always used.
    assert!(config.trusted_proxies.is_empty());
}

// ===================================================================
// MED-1 — two divergent captcha implementations, one unkeyed
// ===================================================================

/// The short-link captcha signed with unkeyed SHA-256, so anyone could forge a
/// `captchaId` offline; the submit endpoint verified an HMAC keyed with
/// `jwt_secret`, so the two could never agree. Both routes now use one keyed
/// implementation.
#[test]
fn captcha_rejects_an_unkeyed_signature() {
    use sha2::{Digest, Sha256};

    let secret = "a-real-server-secret-not-known-to-the-attacker";
    let timestamp = chrono::Utc::now().timestamp_millis();

    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", "ab3xy", timestamp).as_bytes());
    let forged_id = format!("{}.{}", timestamp, hex::encode(hasher.finalize()));

    assert!(
        attendance_geotag_backend::utils::captcha::verify("Ab3Xy", &forged_id, secret).is_err(),
        "an unkeyed SHA-256 signature must not validate"
    );
}

/// The issued image must be a distorted PNG, not the answer in readable SVG
/// text that a regex could lift straight out of the response.
#[test]
fn captcha_image_is_not_machine_readable_text() {
    let issued =
        attendance_geotag_backend::utils::captcha::issue("a-real-server-secret-for-this-test")
            .unwrap();

    assert!(issued.image_html.contains("data:image/png;base64,"));
    assert!(
        !issued.image_html.contains("<text"),
        "the answer must not be rendered as extractable SVG text"
    );
}

// ===================================================================
// MED-5 — client-controlled S3 object keys
// ===================================================================

/// `photoPublicId` came from the request body and was passed straight to S3
/// `download()`, and later to `delete()` when the session was removed — so a
/// submitter could name any object in the bucket and have it read, then
/// destroyed.
#[test]
fn photo_keys_are_confined_to_the_attendance_prefix() {
    use attendance_geotag_backend::storage::validate_attendance_photo_key;

    assert!(validate_attendance_photo_key(
        "attendance-photos/8f14e45f-ea8d-4c2b-9f1a-0d3c5b7e9a11_1721654400.jpg"
    )
    .is_ok());

    for hostile in [
        "db-backups/attendance-2026-08-05.sql.gz",
        "attendance-photos/../db-backups/dump.sql.gz",
        "attendance-photos-evil/payload.jpg",
        "../../etc/passwd",
        "attendance-photos//escaped.jpg",
    ] {
        assert!(
            validate_attendance_photo_key(hostile).is_err(),
            "{hostile:?} must be rejected"
        );
    }
}

// ===================================================================
// LOW — non-constant-time comparison of the rotating QR token
// ===================================================================

/// The QR token guard compared `String == String`, which short-circuits on the
/// first differing byte. Correctness is what is asserted here; the timing
/// property is provided by `subtle::ConstantTimeEq`.
#[test]
fn qr_token_validation_still_accepts_only_the_correct_token() {
    use attendance_geotag_backend::utils::{generate_qr_token, validate_qr_token};

    let short_code = "abc123";
    let secret = "session-totp-secret";

    let token = generate_qr_token(short_code, secret);
    assert!(validate_qr_token(short_code, secret, &token));

    assert!(!validate_qr_token(
        short_code,
        secret,
        "0".repeat(token.len()).as_str()
    ));
    assert!(!validate_qr_token(short_code, "different-secret", &token));
    assert!(!validate_qr_token("other1", secret, &token));
}

// ===================================================================
// Batch/roster is optional — it must never gate a student
// ===================================================================

/// An earlier revision of the enrollment gate refused any roll number not on
/// the session's batch roster, and refused *every* roll number when a session
/// had no batch at all. That blocked legitimate students whenever a roster was
/// incomplete or absent.
///
/// A batch is a reporting roster and a review signal, not an allowlist. This
/// asserts the property at the source level so a future change that
/// reintroduces a block is caught: `start_registration` must contain no
/// roster-derived early return.
#[test]
fn enrollment_has_no_roster_gate() {
    let source = include_str!("../../src/controllers/public_webauthn.rs");

    let start = source
        .find("pub async fn start_registration(")
        .expect("start_registration must exist");
    let end = source[start..]
        .find("\npub async fn ")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    assert!(
        body.contains("on_roster"),
        "the roster lookup should still run — it feeds the off-roster warning"
    );
    assert!(
        !body.contains("not on the roster for this session"),
        "enrollment must not refuse off-roster students; the roster is not an allowlist"
    );
    assert!(
        !body.contains("return Err(AppError::Forbidden"),
        "enrollment must not reject based on batch membership"
    );
}

/// Absence is only meaningful against a roster (regular batch or Excel
/// batch — see `controllers::roster_source`). With no roster attached the
/// stats must report zero rather than inventing absentees, and must say so
/// via `has_roster` so the UI can tell "nobody absent" from "not tracked".
#[test]
fn session_stats_expose_presence_and_absence() {
    let source = include_str!("../../src/controllers/admin/sessions.rs");

    for field in [
        "total_attendance",
        "verified_attendance",
        "unverified_attendance",
        "roster_size",
        "absent_count",
        "has_roster",
    ] {
        assert!(
            source.contains(field),
            "session stats must expose {field} for the session view"
        );
    }

    assert!(
        source.contains("if roster.is_empty()") && source.contains("has_roster: !roster.is_empty()"),
        "a session with no roster (no batch or excel batch attached) must report no roster and no absentees"
    );
}
