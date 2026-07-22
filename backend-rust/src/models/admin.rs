use crate::constants::ROLE_ADMIN;
use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Admin {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub username: String,
    pub email: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub failed_login_attempts: i32,
    pub lock_until: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub created_at: DateTime<Utc>,
}

fn default_role() -> String {
    ROLE_ADMIN.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordHashType {
    Bcrypt,
    Argon2id,
    Unknown,
}

impl Admin {
    pub fn collection_name() -> &'static str {
        "admins"
    }

    pub fn hash_password(password: &str) -> crate::error::Result<String> {
        use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher};
        use rand::Rng;

        let mut salt_bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut salt_bytes);
        let salt = SaltString::encode_b64(&salt_bytes)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::default(),
        );

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        Ok(hash.to_string())
    }

    pub fn detect_hash_type(hash: &str) -> PasswordHashType {
        if hash.starts_with("$2b$") || hash.starts_with("$2a$") || hash.starts_with("$2y$") {
            PasswordHashType::Bcrypt
        } else if hash.starts_with("$argon2") {
            PasswordHashType::Argon2id
        } else {
            PasswordHashType::Unknown
        }
    }

    pub fn verify_password(&self, password: &str) -> crate::error::Result<bool> {
        match Self::detect_hash_type(&self.password) {
            PasswordHashType::Bcrypt => self.verify_bcrypt_password(password),
            PasswordHashType::Argon2id => self.verify_argon2_password(password),
            PasswordHashType::Unknown => Err(crate::error::AppError::Internal(
                "Unknown password hash format".to_string(),
            )),
        }
    }

    fn verify_bcrypt_password(&self, password: &str) -> crate::error::Result<bool> {
        bcrypt::verify(password, &self.password)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))
    }

    fn verify_argon2_password(&self, password: &str) -> crate::error::Result<bool> {
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Algorithm, Argon2, Params,
        };

        let parsed_hash = PasswordHash::new(&self.password)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::default(),
        );

        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    pub fn should_rehash(&self) -> bool {
        matches!(
            Self::detect_hash_type(&self.password),
            PasswordHashType::Bcrypt
        )
    }

    pub fn is_locked(&self) -> bool {
        if let Some(lock_until) = self.lock_until {
            return lock_until > Utc::now();
        }
        false
    }

    pub const MAX_LOGIN_ATTEMPTS: i32 = 5;
    pub const LOCK_TIME_MINUTES: i64 = 15;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRegistration {
    pub username: String,
    pub email: String,
    pub password: String,
    pub admin_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLogin {
    pub username: String,
    pub password: String,
}
