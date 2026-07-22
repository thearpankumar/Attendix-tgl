use chrono::{DateTime, Duration, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnChallenge {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub student_id: String,
    pub challenge: String,
    #[serde(rename = "type")]
    pub challenge_type: WebAuthnChallengeType,
    pub session_id: ObjectId,
    pub short_code: Option<String>,
    pub student_name: Option<String>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub used: bool,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnChallengeType {
    #[serde(rename = "registration")]
    Registration,
    #[serde(rename = "authentication")]
    Authentication,
}

impl WebAuthnChallenge {
    pub fn collection_name() -> &'static str {
        "webauthnchallenges"
    }

    pub fn new(
        student_id: String,
        challenge_type: WebAuthnChallengeType,
        session_id: ObjectId,
    ) -> Self {
        Self {
            id: None,
            student_id,
            challenge: Self::generate_challenge(),
            challenge_type,
            session_id,
            short_code: None,
            student_name: None,
            expires_at: Utc::now() + Duration::minutes(5),
            used: false,
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
