use anyhow::{bail, Context};
use serde::Deserialize;
use std::env;

/// Minimum accepted length for a secret used as an HMAC key or a shared
/// registration secret. `openssl rand -base64 48` produces 64 characters.
const MIN_SECRET_LEN: usize = 32;

/// Substrings that identify a placeholder shipped in `.env.example`, or a
/// fallback that a previous version of this file used to invent silently.
/// A secret containing any of these is refused outright — the whole point of
/// this check is that copying `.env.example` to `.env` must fail loudly rather
/// than boot a service whose signing key is public knowledge.
const PLACEHOLDER_MARKERS: &[&str] = &[
    "change-this",
    "change_this",
    "changeme",
    "dev-secret",
    "dev-admin-secret",
    "your-secret",
    "placeholder",
    "test-key",
    "test-secret",
];

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub pg_max_pool_size: u32,
    pub pg_min_pool_size: u32,
    pub jwt_secret: String,
    pub jwt_expire: String,
    pub admin_secret: String,
    pub node_env: String,
    pub cors_origin: String,
    /// Origin the site is served from, used to build shareable links
    /// (`{public_base_url}/s/{code}`). Distinct from `webauthn.origin`, which
    /// is the WebAuthn relying-party origin — the short links were built from
    /// that, so they pointed at the backend's own port (`:5000`) rather than
    /// the address a student's browser can actually reach through the proxy.
    pub public_base_url: String,
    /// CIDRs whose `X-Forwarded-For` header is trusted for client-IP
    /// extraction. Empty means "trust nobody" — the socket peer address is
    /// used instead. See `middleware::rate_limit_middleware::client_ip`.
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub storage: StorageConfig,
    pub redis: RedisConfig,
    pub webauthn: WebAuthnConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub provider: String,
    pub s3: S3Config,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebAuthnConfig {
    pub rp_name: String,
    pub rp_id: String,
    pub origin: String,
}

/// Reads a required environment variable, rejecting empty values.
fn require_env(key: &str) -> anyhow::Result<String> {
    let value = env::var(key)
        .with_context(|| format!("{key} must be set (see .env.example); there is no default"))?;
    if value.trim().is_empty() {
        bail!("{key} is set but empty");
    }
    Ok(value)
}

/// Reads a required secret and refuses weak or placeholder values.
///
/// This is deliberately unconditional rather than gated on `NODE_ENV`. The
/// previous implementation only enforced secrets when `NODE_ENV == "production"`
/// exactly, so any deployment that left the variable unset — or spelled it
/// `prod` — silently signed tokens with a hardcoded literal.
fn require_secret(key: &str) -> anyhow::Result<String> {
    let value = require_env(key)?;

    // Checked before the length requirement: a short placeholder (e.g. the
    // pre-hardening literal "dev-secret-change-in-production", 31 chars) is
    // still a placeholder first and foremost — "this looks like a
    // placeholder" is more actionable than "this is too short", since
    // padding a placeholder out to 32 characters would otherwise satisfy
    // the length check while still being a known-public string.
    let lowered = value.to_ascii_lowercase();
    if let Some(marker) = PLACEHOLDER_MARKERS
        .iter()
        .find(|marker| lowered.contains(*marker))
    {
        bail!(
            "{key} looks like a placeholder (contains {marker:?}). \
             Generate a real one with: openssl rand -base64 48"
        );
    }

    if value.len() < MIN_SECRET_LEN {
        bail!(
            "{key} is only {} characters; at least {MIN_SECRET_LEN} are required. \
             Generate one with: openssl rand -base64 48",
            value.len()
        );
    }

    Ok(value)
}

fn parse_trusted_proxies(raw: &str) -> anyhow::Result<Vec<ipnet::IpNet>> {
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .parse::<ipnet::IpNet>()
                // Accept a bare address as a single-host network so operators
                // can write `10.0.0.5` instead of `10.0.0.5/32`.
                .or_else(|_| entry.parse::<std::net::IpAddr>().map(ipnet::IpNet::from))
                .with_context(|| {
                    format!("TRUSTED_PROXIES entry {entry:?} is not a valid IP or CIDR")
                })
        })
        .collect()
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let node_env = env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string());

        let jwt_secret = require_secret("JWT_SECRET")?;
        let admin_secret = require_secret("ADMIN_SECRET")?;

        // A wildcard origin is never correct for this API: the admin frontend
        // sends a bearer token and the student frontend is same-origin.
        let cors_origin = require_env("CORS_ORIGIN")?;
        if cors_origin.trim() == "*" {
            bail!(
                "CORS_ORIGIN=* is not accepted. List the exact origins, \
                 comma-separated (e.g. https://attendix.example.com)"
            );
        }

        // Trailing slashes would produce `https://host//s/code`.
        let public_base_url = require_env("PUBLIC_BASE_URL")?
            .trim_end_matches('/')
            .to_string();

        let provider = env::var("STORAGE_PROVIDER").unwrap_or_else(|_| "s3".to_string());
        let s3 = if provider == "s3" {
            S3Config {
                bucket: require_env("AWS_S3_BUCKET")?,
                region: env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
                access_key_id: require_env("AWS_ACCESS_KEY_ID")?,
                secret_access_key: require_secret("AWS_SECRET_ACCESS_KEY")?,
            }
        } else {
            bail!("STORAGE_PROVIDER={provider:?} is not supported; only \"s3\" is implemented");
        };

        let webauthn_origin = require_env("WEBAUTHN_ORIGIN")?;
        let webauthn_rp_id = require_env("WEBAUTHN_RP_ID")?;
        // WebAuthn requires a secure context. Browsers make an exception for
        // localhost, so allow plain HTTP only there.
        if webauthn_origin.starts_with("http://")
            && !webauthn_origin.starts_with("http://localhost")
            && !webauthn_origin.starts_with("http://127.0.0.1")
        {
            bail!(
                "WEBAUTHN_ORIGIN={webauthn_origin:?} uses plain HTTP. \
                 WebAuthn requires https:// outside of localhost"
            );
        }

        // Every shared-state service (rate limiter, deny-list, session cache,
        // GPS anomaly detection) is Redis-backed with no in-memory fallback —
        // a missing REDIS_URL must fail startup, not silently degrade a
        // multi-replica deployment down to per-process state.
        let redis_url = require_env("REDIS_URL")?;

        Ok(AppConfig {
            port: env::var("PORT")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .context("PORT must be a number")?,
            database_url: require_env("DATABASE_URL")?,
            pg_max_pool_size: env::var("PG_MAX_POOL_SIZE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .context("PG_MAX_POOL_SIZE must be a number")?,
            pg_min_pool_size: env::var("PG_MIN_POOL_SIZE")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("PG_MIN_POOL_SIZE must be a number")?,
            jwt_secret,
            jwt_expire: env::var("JWT_EXPIRE").unwrap_or_else(|_| "1d".to_string()),
            admin_secret,
            node_env,
            cors_origin,
            public_base_url,
            trusted_proxies: parse_trusted_proxies(
                &env::var("TRUSTED_PROXIES").unwrap_or_default(),
            )?,
            storage: StorageConfig { provider, s3 },
            redis: RedisConfig { url: redis_url },
            webauthn: WebAuthnConfig {
                rp_name: env::var("WEBAUTHN_RP_NAME")
                    .unwrap_or_else(|_| "Attendix Attendance System".to_string()),
                rp_id: webauthn_rp_id,
                origin: webauthn_origin,
            },
        })
    }

    pub fn is_production(&self) -> bool {
        self.node_env == "production"
    }

    /// Fixed configuration for the integration test suite.
    ///
    /// Gated behind the `integration-tests` feature so these values cannot be
    /// compiled into the production binary — there is deliberately no `Default`
    /// impl, because `AppConfig::default()` was previously reachable from
    /// production code and handed out a hardcoded JWT signing key.
    #[cfg(feature = "integration-tests")]
    pub fn for_testing() -> Self {
        AppConfig {
            port: 5000,
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/attendance_geotag_test".to_string()
            }),
            pg_max_pool_size: 5,
            pg_min_pool_size: 1,
            jwt_secret: "integration-test-signing-key-not-for-any-real-deployment".to_string(),
            jwt_expire: "1d".to_string(),
            admin_secret: "integration-test-admin-secret-not-for-any-real-deployment".to_string(),
            node_env: "test".to_string(),
            cors_origin: "http://localhost:5173".to_string(),
            public_base_url: "http://localhost".to_string(),
            trusted_proxies: Vec::new(),
            storage: StorageConfig {
                provider: "s3".to_string(),
                s3: S3Config {
                    bucket: "integration-test-bucket".to_string(),
                    region: "us-east-1".to_string(),
                    access_key_id: "integration-test-key".to_string(),
                    secret_access_key: "integration-test-secret".to_string(),
                },
            },
            redis: RedisConfig {
                url: env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            },
            webauthn: WebAuthnConfig {
                rp_name: "Attendix Test".to_string(),
                rp_id: "localhost".to_string(),
                origin: "http://localhost:5000".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_secrets() {
        let err = require_secret_value("JWT_SECRET", "tooshort").unwrap_err();
        assert!(err.to_string().contains("at least"));
    }

    #[test]
    fn rejects_the_env_example_placeholder() {
        let err = require_secret_value("JWT_SECRET", "CHANGE-THIS-TO-A-SECURE-RANDOM-STRING")
            .unwrap_err();
        assert!(err.to_string().contains("placeholder"));
    }

    /// The exact literal the previous implementation used as a silent fallback.
    #[test]
    fn rejects_the_old_hardcoded_fallback() {
        let err =
            require_secret_value("JWT_SECRET", "dev-secret-change-in-production").unwrap_err();
        assert!(err.to_string().contains("placeholder"));
    }

    #[test]
    fn accepts_a_generated_secret() {
        // Representative `openssl rand -base64 48` output.
        let generated = "kQ7dP2xLm9vRt4bYw8zNc1FhJ6aGsEuIoT3rXpVdMk5CyBnQwZlA0HgSfUjOePiT";
        assert!(require_secret_value("JWT_SECRET", generated).is_ok());
    }

    #[test]
    fn parses_cidrs_and_bare_addresses() {
        let nets = parse_trusted_proxies("172.18.0.0/16, 10.0.0.5").unwrap();
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains(&"172.18.0.7".parse::<std::net::IpAddr>().unwrap()));
        assert!(nets[1].contains(&"10.0.0.5".parse::<std::net::IpAddr>().unwrap()));
    }

    #[test]
    fn rejects_a_malformed_proxy_cidr() {
        assert!(parse_trusted_proxies("not-an-ip").is_err());
    }

    /// Mirrors `require_secret` without touching process-global env, so these
    /// tests stay independent of execution order.
    fn require_secret_value(key: &str, value: &str) -> anyhow::Result<String> {
        // Mirrors require_secret's check order above: placeholder before length.
        let lowered = value.to_ascii_lowercase();
        if let Some(marker) = PLACEHOLDER_MARKERS
            .iter()
            .find(|marker| lowered.contains(*marker))
        {
            bail!("{key} looks like a placeholder (contains {marker:?})");
        }
        if value.len() < MIN_SECRET_LEN {
            bail!(
                "{key} is only {} characters; at least {MIN_SECRET_LEN} are required",
                value.len()
            );
        }
        Ok(value.to_string())
    }
}
