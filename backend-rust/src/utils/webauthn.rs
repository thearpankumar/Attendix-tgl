use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use webauthn_rs::prelude::*;

pub struct WebAuthnConfig {
    rp_name: String,
    _rp_id: String,
    origin: String,
}

impl WebAuthnConfig {
    pub fn new(rp_name: String, _rp_id: String, origin: String) -> Self {
        Self {
            rp_name,
            _rp_id,
            origin,
        }
    }

    pub fn create_webauthn(&self) -> crate::error::Result<Webauthn> {
        let url = url::Url::parse(&self.origin)
            .map_err(|e| crate::error::AppError::Internal(format!("URL parse error: {}", e)))?;
        let rp_id = url.domain().unwrap_or("localhost").to_string();

        let builder = WebauthnBuilder::new(&rp_id, &url).map_err(|e| {
            crate::error::AppError::Internal(format!("WebAuthn builder error: {:?}", e))
        })?;

        builder
            .build()
            .map_err(|e| crate::error::AppError::Internal(format!("WebAuthn build error: {:?}", e)))
    }

    pub fn rp_name(&self) -> &str {
        &self.rp_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnRegistrationStart {
    pub challenge: String,
    pub options: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnAuthenticationStart {
    pub challenge: String,
    pub options: String,
}

pub fn generate_challenge() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    BASE64_STANDARD.encode(bytes)
}
