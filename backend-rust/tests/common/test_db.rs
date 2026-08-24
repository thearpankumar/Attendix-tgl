use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;

// `tokio::sync::OnceCell` rather than `std::sync::OnceLock`: the latter's
// `get_or_init` is synchronous, which previously forced this through
// `block_in_place` — a call that panics outside a multi-threaded Tokio
// runtime, which `#[tokio::test]` doesn't use by default.
static TEST_ENV: OnceCell<TestEnvironment> = OnceCell::const_new();

pub struct TestEnvironment {
    // Held so the handle stays alive for the process lifetime, but note these
    // are `with_reuse(Always)` containers (see below) -- they do NOT stop on
    // drop/process exit by design. Every `[[test]]` binary is its own OS
    // process (so this `static` is NOT actually shared across e.g.
    // `tests/security` and `tests/middleware`), and without a fixed name +
    // reuse, each of the 5 binaries used to start its own throwaway
    // Postgres+Redis pair every single `cargo test` run, and none of them
    // ever got cleaned up (relies on the `ryuk` reaper container, which
    // wasn't running in the environment these were leaking in) -- so a full
    // suite run, repeated a few times, left dozens of orphaned containers
    // behind. Reuse collapses that to exactly 2 containers total, shared by
    // name across every binary and every run, never multiplying.
    //
    // Trade-off: since `cargo test` runs different `[[test]]` binaries
    // concurrently by default, they now share one live database/redis
    // instance instead of each getting a fully isolated one -- tests that
    // read/write global state here should account for that (via
    // `cleanup_test_db` or their own scoping) rather than assuming exclusive
    // access the way a fresh-per-binary container used to guarantee.
    #[allow(dead_code)]
    pub postgres_container: testcontainers::ContainerAsync<Postgres>,
    #[allow(dead_code)]
    pub redis_container: testcontainers::ContainerAsync<Redis>,
    #[allow(dead_code)]
    pub timescaledb_container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
    pub database_url: String,
    pub redis_uri: String,
    pub timescale_database_url: String,
}

// `with_reuse(Always)` races across the ~7 separate test-binary processes
// nextest spawns concurrently: if two processes both see "no container named
// backend-rust-test-postgres yet" at the same moment, both call `docker
// create --name backend-rust-test-postgres`, and only one wins — the loser
// gets a hard 409 Conflict from the Docker daemon instead of the library
// falling back to reuse. Retrying the whole `start()` call is the fix: by
// the next attempt the winner's container already exists (even if still
// starting up — the reuse path waits on the same readiness strategy), so
// the reuse lookup finds and attaches to it instead of trying to create it
// again.
const CONTAINER_START_RETRIES: u32 = 15;
const CONTAINER_START_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

impl TestEnvironment {
    pub async fn new() -> Self {
        use testcontainers::{runners::AsyncRunner, ImageExt, ReuseDirective};

        // testcontainers-modules' default Postgres image is 11-alpine, which
        // predates core `gen_random_uuid()` (added in Postgres 13) — every
        // `id UUID PRIMARY KEY DEFAULT gen_random_uuid()` column in the
        // migrations failed to apply against this container with "function
        // gen_random_uuid() does not exist" until pgcrypto (which backports
        // it) is installed. The prod/dev image (postgres:18-alpine, see
        // docker-compose.yml) doesn't need this — it's new enough already.
        let mut last_err = None;
        let mut postgres_container = None;
        for attempt in 0..CONTAINER_START_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(CONTAINER_START_RETRY_DELAY).await;
            }
            match Postgres::default()
                .with_init_sql(
                    "CREATE EXTENSION IF NOT EXISTS pgcrypto;"
                        .to_string()
                        .into_bytes(),
                )
                .with_container_name("backend-rust-test-postgres")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
            {
                Ok(c) => {
                    postgres_container = Some(c);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let postgres_container = postgres_container.unwrap_or_else(|| {
            panic!(
                "Failed to start Postgres container after {CONTAINER_START_RETRIES} attempts: {:?}",
                last_err
            )
        });

        let mut last_err = None;
        let mut redis_container = None;
        for attempt in 0..CONTAINER_START_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(CONTAINER_START_RETRY_DELAY).await;
            }
            match Redis::default()
                .with_container_name("backend-rust-test-redis")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
            {
                Ok(c) => {
                    redis_container = Some(c);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let redis_container = redis_container.unwrap_or_else(|| {
            panic!(
                "Failed to start Redis container after {CONTAINER_START_RETRIES} attempts: {:?}",
                last_err
            )
        });

        // TimescaleDB's image ships no dedicated testcontainers-modules
        // wrapper, so this is a GenericImage configured to match the same
        // startup-log wait strategy the Postgres module above uses (the
        // TimescaleDB image is a superset of the official Postgres image and
        // logs identically on boot).
        use testcontainers::{core::WaitFor, GenericImage};
        let mut last_err = None;
        let mut timescaledb_container = None;
        for attempt in 0..CONTAINER_START_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(CONTAINER_START_RETRY_DELAY).await;
            }
            match GenericImage::new("timescale/timescaledb", "latest-pg17")
                .with_exposed_port(5432.into())
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_wait_for(WaitFor::message_on_stdout(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_DB", "postgres")
                .with_env_var("POSTGRES_USER", "postgres")
                .with_env_var("POSTGRES_PASSWORD", "postgres")
                .with_container_name("backend-rust-test-timescaledb")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
            {
                Ok(c) => {
                    timescaledb_container = Some(c);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let timescaledb_container = timescaledb_container.unwrap_or_else(|| {
            panic!(
                "Failed to start TimescaleDB container after {CONTAINER_START_RETRIES} attempts: {:?}",
                last_err
            )
        });

        let postgres_port = postgres_container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get Postgres port");

        let redis_port = redis_container
            .get_host_port_ipv4(6379)
            .await
            .expect("Failed to get Redis port");

        let timescaledb_port = timescaledb_container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get TimescaleDB port");

        let database_url = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            postgres_port
        );
        let redis_uri = format!("redis://localhost:{}", redis_port);
        let timescale_database_url = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            timescaledb_port
        );

        Self {
            postgres_container,
            redis_container,
            timescaledb_container,
            database_url,
            redis_uri,
            timescale_database_url,
        }
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn redis_uri(&self) -> &str {
        &self.redis_uri
    }

    pub fn timescale_database_url(&self) -> &str {
        &self.timescale_database_url
    }
}

pub async fn get_test_environment() -> &'static TestEnvironment {
    TEST_ENV.get_or_init(TestEnvironment::new).await
}

/// Connects to the shared test Postgres container and applies migrations,
/// returning a pool ready for use. Each call reuses the same underlying
/// database (Postgres testcontainers only expose one DB per container) —
/// callers that need isolation should clean up their own rows/tables.
#[allow(dead_code)]
pub async fn get_test_database() -> sqlx::PgPool {
    let env = get_test_environment().await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(env.database_url())
        .await
        .expect("Failed to connect to test Postgres database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations against test database");

    pool
}

/// Same as `get_test_database`, but against the separate TimescaleDB
/// container (see `AppState::timescale_db`) with its own migration set.
#[allow(dead_code)]
pub async fn get_test_timescale_database() -> sqlx::PgPool {
    let env = get_test_environment().await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(env.timescale_database_url())
        .await
        .expect("Failed to connect to test TimescaleDB database");

    sqlx::migrate!("./migrations_timescale")
        .run(&pool)
        .await
        .expect("Failed to run migrations against test TimescaleDB database");

    pool
}

#[allow(dead_code)]
pub async fn get_test_redis() -> redis::Connection {
    let env = get_test_environment().await;

    redis::Client::open(env.redis_uri())
        .expect("Failed to create Redis client")
        .get_connection()
        .expect("Failed to get Redis connection")
}

/// Truncates all tables so the shared test database starts clean for the next test.
#[allow(dead_code)]
pub async fn cleanup_test_db(pool: &sqlx::PgPool) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "TRUNCATE TABLE admins, locations, batches, students, sessions, attendances, \
             devices, device_fingerprints, flags, photo_hashes, short_links, system_configs, \
             webauthn_challenges, webauthn_credentials, webauthn_reenrollment_logs, \
             recurring_session_rules, extension_installations, session_device_locks \
             RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_environment_starts() {
        let env = get_test_environment().await;
        assert!(!env.database_url().is_empty());
        assert!(!env.redis_uri().is_empty());
    }
}
