//! Stateless captcha issuing and verification.
//!
//! There used to be two implementations. The one behind `/api/attend/{token}/captcha`
//! rendered a distorted PNG and signed the answer with `HMAC-SHA256(jwt_secret)`.
//! The one behind `/api/s/{shortCode}/captcha` rendered the answer as plain SVG
//! `<text>` — readable with a regex, no OCR needed — and signed it with *unkeyed*
//! SHA-256, so anyone could forge a `captchaId` for any answer offline. The two
//! could never validate against each other. Both routes now call this module.
//!
//! The scheme is deliberately stateless: `captcha_id` is `timestamp.HMAC(answer:timestamp)`,
//! so no server-side store is needed. The trade-off is that a captcha is replayable
//! within its validity window; `CAPTCHA_VALIDITY` keeps that window short.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::{AppError, Result};

/// How long an issued captcha stays valid.
const CAPTCHA_VALIDITY_MS: i64 = 5 * 60 * 1000;

/// Number of characters in the challenge.
const CAPTCHA_LEN: u32 = 5;

pub struct IssuedCaptcha {
    /// An `<img>` element wrapping a base64 PNG, ready to embed.
    pub image_html: String,
    /// `timestamp.signature`, echoed back by the client on submit.
    pub captcha_id: String,
}

/// Renders a distorted captcha and returns it with its signed identifier.
pub fn issue(jwt_secret: &str) -> Result<IssuedCaptcha> {
    use captcha::{
        filters::{Dots, Noise, Wave},
        Captcha,
    };

    let mut captcha = Captcha::new();
    captcha.add_chars(CAPTCHA_LEN);
    let answer = captcha.chars_as_string();

    // Distortion is what makes the challenge non-trivial to read
    // programmatically — the plain-SVG variant this replaced had none.
    captcha
        .apply_filter(Noise::new(0.4))
        .apply_filter(Wave::new(2.0, 20.0).horizontal())
        .view(220, 120)
        .apply_filter(Dots::new(15));

    let png = captcha
        .as_png()
        .ok_or_else(|| AppError::Internal("Failed to render captcha image".to_string()))?;

    let timestamp = chrono::Utc::now().timestamp_millis();

    Ok(IssuedCaptcha {
        image_html: format!(
            "<img src=\"data:image/png;base64,{}\" alt=\"captcha\" />",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png)
        ),
        captcha_id: format!("{}.{}", timestamp, sign(&answer, timestamp, jwt_secret)?),
    })
}

/// Verifies a submitted answer against its signed identifier.
pub fn verify(answer: &str, captcha_id: &str, jwt_secret: &str) -> Result<()> {
    let (timestamp_part, signature) = captcha_id
        .split_once('.')
        .ok_or_else(|| AppError::BadRequest("Invalid captcha ID format".to_string()))?;

    let timestamp: i64 = timestamp_part
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid captcha timestamp".to_string()))?;

    let age = chrono::Utc::now().timestamp_millis() - timestamp;
    // Reject future timestamps too: a client that can pick its own timestamp
    // could otherwise mint a captcha that stays valid indefinitely.
    if !(0..=CAPTCHA_VALIDITY_MS).contains(&age) {
        return Err(AppError::BadRequest(
            "Captcha expired. Please refresh and try again.".to_string(),
        ));
    }

    let expected = sign(answer, timestamp, jwt_secret)?;

    // Constant-time: a byte-wise `==` on a hex MAC leaks the correct prefix
    // through timing, and this endpoint accepts unlimited guesses per captcha.
    if expected.as_bytes().ct_eq(signature.as_bytes()).into() {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "Incorrect captcha. Please try again.".to_string(),
        ))
    }
}

fn sign(answer: &str, timestamp: i64, jwt_secret: &str) -> Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(jwt_secret.as_bytes())
        .map_err(|_| AppError::Internal("Failed to create HMAC".to_string()))?;
    mac.update(format!("{}:{}", answer.to_lowercase(), timestamp).as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-signing-key-for-captcha-unit-tests-only";

    #[test]
    fn round_trips_a_freshly_issued_captcha() {
        // `issue` does not reveal its answer, so sign directly to build the pair.
        let timestamp = chrono::Utc::now().timestamp_millis();
        let captcha_id = format!(
            "{}.{}",
            timestamp,
            sign("Ab3Xy", timestamp, SECRET).unwrap()
        );
        assert!(verify("Ab3Xy", &captcha_id, SECRET).is_ok());
    }

    #[test]
    fn accepts_any_letter_case() {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let captcha_id = format!(
            "{}.{}",
            timestamp,
            sign("Ab3Xy", timestamp, SECRET).unwrap()
        );
        assert!(verify("aB3xY", &captcha_id, SECRET).is_ok());
    }

    #[test]
    fn rejects_a_wrong_answer() {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let captcha_id = format!(
            "{}.{}",
            timestamp,
            sign("Ab3Xy", timestamp, SECRET).unwrap()
        );
        assert!(verify("WRONG", &captcha_id, SECRET).is_err());
    }

    /// The whole point of keying the MAC: an attacker who knows the answer must
    /// still be unable to produce a valid identifier without the server secret.
    #[test]
    fn rejects_an_identifier_signed_with_a_different_secret() {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let forged = format!(
            "{}.{}",
            timestamp,
            sign("Ab3Xy", timestamp, "attacker-controlled-secret").unwrap()
        );
        assert!(verify("Ab3Xy", &forged, SECRET).is_err());
    }

    /// Regression: the superseded short-link implementation signed with plain,
    /// unkeyed SHA-256, which anyone could recompute.
    #[test]
    fn rejects_an_unkeyed_sha256_signature() {
        use sha2::Digest;
        let timestamp = chrono::Utc::now().timestamp_millis();
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", "ab3xy", timestamp).as_bytes());
        let forged = format!("{}.{}", timestamp, hex::encode(hasher.finalize()));
        assert!(verify("Ab3Xy", &forged, SECRET).is_err());
    }

    #[test]
    fn rejects_an_expired_captcha() {
        let stale = chrono::Utc::now().timestamp_millis() - CAPTCHA_VALIDITY_MS - 1;
        let captcha_id = format!("{}.{}", stale, sign("Ab3Xy", stale, SECRET).unwrap());
        assert!(verify("Ab3Xy", &captcha_id, SECRET).is_err());
    }

    #[test]
    fn rejects_a_future_timestamp() {
        let future = chrono::Utc::now().timestamp_millis() + 60_000;
        let captcha_id = format!("{}.{}", future, sign("Ab3Xy", future, SECRET).unwrap());
        assert!(verify("Ab3Xy", &captcha_id, SECRET).is_err());
    }

    #[test]
    fn rejects_a_malformed_identifier() {
        assert!(verify("Ab3Xy", "no-separator", SECRET).is_err());
    }

    #[test]
    fn issues_a_png_backed_image_and_a_signed_id() {
        let issued = issue(SECRET).unwrap();
        assert!(issued.image_html.contains("data:image/png;base64,"));
        let (ts, sig) = issued.captcha_id.split_once('.').unwrap();
        assert!(ts.parse::<i64>().is_ok());
        assert_eq!(sig.len(), 64, "HMAC-SHA256 hex digest is 64 characters");
    }
}
