use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use attendance_geotag_backend::{
    config::AppConfig,
    middleware::{RateLimiter, SessionCache},
    models::SystemConfig,
    services::GpsHistoryService,
    storage::Storage,
    AppState,
};
use tokio::sync::RwLock;

fn init_tracing() {
    let is_dev = std::env::var("NODE_ENV").unwrap_or_default() == "development"
        || std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug");

    if is_dev {
        // Development mode: Pretty-printed logs
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into()),
            ))
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_thread_names(false),
            )
            .init();
    } else {
        // Production mode: JSON logs
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
            ))
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    init_tracing();

    let config = AppConfig::from_env()?;

    tracing::info!("Connecting to PostgreSQL...");

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.pg_max_pool_size)
        .min_connections(config.pg_min_pool_size)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let redis_client = if !config.redis.url.is_empty() {
        match redis::Client::open(config.redis.url.as_str()) {
            Ok(client) => {
                tracing::info!("Redis client initialized successfully");
                Some(Arc::new(client))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize Redis client: {}. Falling back to in-memory caching.",
                    e
                );
                None
            }
        }
    } else {
        tracing::info!("REDIS_URL not set, using in-memory caching");
        None
    };

    let rate_limiter = Arc::new(RateLimiter::with_redis(redis_client.clone()));
    let session_cache = Arc::new(SessionCache::new(redis_client.clone(), 300));
    let gps_history = Arc::new(GpsHistoryService::new(redis_client.clone()));

    // Initialize AWS SDK config with HTTP timeouts for S3 operations
    // The AWS SDK uses its own HTTP client with built-in timeouts:
    // - v2026_01_12 enables retries by default and sets 3.1s connect timeout
    // S3 operations will use these SDK-level timeouts
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::v2026_01_12())
        .load()
        .await;

    let storage = Storage::new(&aws_config, &config.storage)?;

    // HTTP client for external API calls (IP lookup, etc.)
    // Configured with explicit timeouts to prevent hanging connections
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("Attendance-GEOTAG-Backend/1.0")
        .build()?;

    // Load system config from DB on startup for hot-reload cache
    let system_config_startup = match SystemConfig::load(&db).await {
        Ok(Some(cfg)) => {
            tracing::info!("System config loaded from DB");
            cfg
        }
        Ok(None) => {
            tracing::info!("No system config in DB, using defaults");
            SystemConfig::default()
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load system config from DB: {}. Using defaults.",
                e
            );
            SystemConfig::default()
        }
    };

    if redis_client.is_some() {
        tracing::info!("Redis-backed services enabled:");
        tracing::info!("  - Session caching: {}", session_cache.is_redis_enabled());
        tracing::info!("  - Rate limiting: {}", rate_limiter.is_redis_enabled());
        tracing::info!("  - GPS history: {}", gps_history.is_enabled());
    }

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        redis: redis_client.map(|rc| (*rc).clone()),
        rate_limiter,
        session_cache,
        gps_history,
        start_time: std::time::Instant::now(),
        storage,
        http_client,
        system_config: Arc::new(RwLock::new(system_config_startup)),
        webauthn: Arc::new(attendance_geotag_backend::build_webauthn(&config)?),
    });

    spawn_webauthn_challenge_cleanup(state.db.clone());

    // Fail closed: every configured origin must parse. Previously this used
    // `filter_map(..ok())`, so a single typo silently emptied the list and the
    // code below fell through to `allow_origin(Any)`.
    let cors_origins = config
        .cors_origin
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            origin.parse::<axum::http::HeaderValue>().map_err(|_| {
                anyhow::anyhow!("CORS_ORIGIN entry {origin:?} is not a valid origin header")
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if cors_origins.is_empty() {
        anyhow::bail!("CORS_ORIGIN must list at least one origin");
    }

    let cors = CorsLayer::new()
        .allow_origin(cors_origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::PATCH,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ]);

    let app = Router::new()
        .merge(attendance_geotag_backend::routes::create_routes(state))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;

    tracing::info!("Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Mongo's TTL index used to auto-expire `webauthnchallenges` rows 300s after
/// `expiresAt`. Postgres has no native TTL index, so a periodic sweep replaces it.
fn spawn_webauthn_challenge_cleanup(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            match sqlx::query(
                "DELETE FROM webauthn_challenges WHERE expires_at < now() - INTERVAL '300 seconds'",
            )
            .execute(&pool)
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        tracing::debug!(
                            "Cleaned up {} expired webauthn challenges",
                            result.rows_affected()
                        );
                    }
                }
                Err(e) => tracing::warn!("Failed to clean up expired webauthn challenges: {}", e),
            }
        }
    });
}
