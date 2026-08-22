mod admin;
mod attendance;
mod audit_log;
mod batch;
mod device;
mod device_fingerprint;
mod excel_batch;
mod flag;
mod location;
mod photo_hash;
mod recurring_session_rule;
mod session;
mod short_link;
mod system_config;
mod webauthn_challenge;
mod webauthn_credential;
mod webauthn_reenrollment_log;

pub use admin::*;
pub use attendance::*;
pub use audit_log::*;
pub use batch::*;
pub use device::*;
pub use device_fingerprint::*;
pub use excel_batch::*;
pub use flag::*;
pub use location::*;
pub use photo_hash::*;
pub use recurring_session_rule::*;
pub use session::*;
pub use short_link::*;
pub use system_config::*;
pub use webauthn_challenge::{WebAuthnChallenge, WebAuthnChallengeType};
pub use webauthn_credential::*;
pub use webauthn_reenrollment_log::*;

// Re-export Severity from constants for convenience
pub use crate::constants::Severity;

/// Bridges a serde-tagged unit enum (`#[serde(rename = "...")]` per variant) to a
/// Postgres TEXT column, by round-tripping through the enum's own serde impl. This
/// keeps the on-disk string representation identical to the JSON API representation
/// without duplicating the rename table in a second place.
macro_rules! text_enum_sqlx {
    ($ty:ty) => {
        impl sqlx::Type<sqlx::Postgres> for $ty {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <String as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $ty {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                Ok(serde_json::from_value(serde_json::Value::String(s))?)
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $ty {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                let value = serde_json::to_value(self)?;
                let s = value
                    .as_str()
                    .ok_or("enum did not serialize to a string")?
                    .to_string();
                <String as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
            }
        }
    };
}

pub(crate) use text_enum_sqlx;
