use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const QR_TOKEN_VALIDITY_SECS: i64 = 4;
const QR_TOKEN_CHARS: usize = 16;

/// Generate a time-based QR token for anti-sharing
pub fn generate_qr_token(short_code: &str, totp_secret: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let slot = now / QR_TOKEN_VALIDITY_SECS;

    let mut mac =
        Hmac::<Sha256>::new_from_slice(totp_secret.as_bytes()).expect("HMAC initialization failed");
    mac.update(format!("{}:{}", short_code, slot).as_bytes());

    hex::encode(&mac.finalize().into_bytes()[..QR_TOKEN_CHARS])
}

/// Validate a QR token (checks current and previous slot for 8 second window)
pub fn validate_qr_token(short_code: &str, totp_secret: &str, token: &str) -> bool {
    let now = chrono::Utc::now().timestamp();
    let current_slot = now / QR_TOKEN_VALIDITY_SECS;
    let previous_slot = current_slot - 1;

    // Constant-time comparison: a plain `==` on these strings short-circuits on
    // the first differing byte, leaking the correct prefix through response
    // timing. Both slots are always compared so the check takes the same time
    // whether or not the current slot matched.
    use subtle::ConstantTimeEq;

    let current_match: bool = token
        .as_bytes()
        .ct_eq(generate_qr_token_for_slot(short_code, totp_secret, current_slot).as_bytes())
        .into();
    let previous_match: bool = token
        .as_bytes()
        .ct_eq(generate_qr_token_for_slot(short_code, totp_secret, previous_slot).as_bytes())
        .into();

    current_match | previous_match
}

fn generate_qr_token_for_slot(short_code: &str, totp_secret: &str, slot: i64) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(totp_secret.as_bytes()).expect("HMAC initialization failed");
    mac.update(format!("{}:{}", short_code, slot).as_bytes());

    hex::encode(&mac.finalize().into_bytes()[..QR_TOKEN_CHARS])
}
