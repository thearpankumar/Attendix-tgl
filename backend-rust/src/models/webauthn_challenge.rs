use crate::models::text_enum_sqlx;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnChallenge {
    pub id: Uuid,
    pub student_id: String,
    pub challenge: String,
    #[serde(rename = "type")]
    pub challenge_type: WebAuthnChallengeType,
    pub session_id: Uuid,
    pub short_code: Option<String>,
    pub student_name: Option<String>,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub used: bool,
    /// Serialised `webauthn-rs` ceremony state, replayed at finish time.
    pub state: Option<sqlx::types::JsonValue>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnChallengeType {
    #[serde(rename = "registration")]
    Registration,
    #[serde(rename = "authentication")]
    Authentication,
}
text_enum_sqlx!(WebAuthnChallengeType);

impl WebAuthnChallenge {
    pub fn table_name() -> &'static str {
        "webauthn_challenges"
    }

    pub fn new(
        student_id: String,
        challenge_type: WebAuthnChallengeType,
        session_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            student_id,
            challenge: Self::generate_challenge(),
            challenge_type,
            session_id,
            short_code: None,
            student_name: None,
            expires_at: Utc::now() + Duration::minutes(5),
            used: false,
            state: None,
            created_at: Utc::now(),
        }
    }

    fn generate_challenge() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }
}
