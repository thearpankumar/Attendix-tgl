use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub mongodb_uri: String,
    pub mongodb_max_pool_size: u32,
    pub mongodb_min_pool_size: u32,
    pub jwt_secret: String,
    pub jwt_expire: String,
    pub admin_secret: String,
    pub node_env: String,
    pub cors_origin: String,
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
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebAuthnConfig {
    pub rp_name: String,
    pub rp_id: String,
    pub origin: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let node_env = env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string());
        let is_production = node_env == "production";

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            if is_production {
                panic!("JWT_SECRET must be set in production");
            }
            "dev-secret-change-in-production".to_string()
        });

        let admin_secret = env::var("ADMIN_SECRET").unwrap_or_else(|_| {
            if is_production {
                panic!("ADMIN_SECRET must be set in production");
            }
            "dev-admin-secret".to_string()
        });

        let provider = env::var("STORAGE_PROVIDER").unwrap_or_else(|_| "s3".to_string());

        Ok(AppConfig {
            port: env::var("PORT")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()?,
            mongodb_uri: env::var("MONGODB_URI")
                .unwrap_or_else(|_| "mongodb://localhost:27017/attendance-geotag".to_string()),
            mongodb_max_pool_size: env::var("MONGODB_MAX_POOL_SIZE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            mongodb_min_pool_size: env::var("MONGODB_MIN_POOL_SIZE")
                .unwrap_or_else(|_| "5".to_string())
                .parse()?,
            jwt_secret,
            jwt_expire: env::var("JWT_EXPIRE").unwrap_or_else(|_| "7d".to_string()),
            admin_secret,
            node_env,
            cors_origin: env::var("CORS_ORIGIN").unwrap_or_else(|_| "*".to_string()),
            storage: StorageConfig {
                provider: provider.clone(),
                s3: {
                    let bucket = env::var("AWS_S3_BUCKET").unwrap_or_default();
                    let access_key_id = env::var("AWS_ACCESS_KEY_ID").unwrap_or_default();
                    let secret_access_key = env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default();

                    if is_production {
                        if bucket.is_empty() {
                            panic!("AWS_S3_BUCKET must be set in production");
                        }
                        if access_key_id.is_empty() {
                            panic!("AWS_ACCESS_KEY_ID must be set in production");
                        }
                        if secret_access_key.is_empty() {
                            panic!("AWS_SECRET_ACCESS_KEY must be set in production");
                        }
                    }

                    S3Config {
                        bucket,
                        region: env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
                        access_key_id,
                        secret_access_key,
                    }
                },
            },
            redis: RedisConfig {
                url: env::var("REDIS_URL").unwrap_or_default(),
                enabled: env::var("REDIS_URL").is_ok(),
            },
            webauthn: WebAuthnConfig {
                rp_name: env::var("WEBAUTHN_RP_NAME")
                    .unwrap_or_else(|_| "Attendix Attendance System".to_string()),
                rp_id: env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string()),
                origin: env::var("WEBAUTHN_ORIGIN")
                    .unwrap_or_else(|_| "http://localhost:5000".to_string()),
            },
        })
    }

    pub fn is_production(&self) -> bool {
        self.node_env == "production"
    }
}
