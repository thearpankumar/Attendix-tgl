use std::sync::OnceLock;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

static TEST_ENV: OnceLock<TestEnvironment> = OnceLock::new();

pub struct TestEnvironment {
    pub postgres_container: testcontainers::Container<Postgres>,
    pub redis_container: testcontainers::Container<Redis>,
    pub database_url: String,
    pub redis_uri: String,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        use testcontainers::runners::AsyncRunner;

        let postgres_container = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let redis_container = Redis::default()
            .start()
            .await
            .expect("Failed to start Redis container");

        let postgres_port = postgres_container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get Postgres port");

        let redis_port = redis_container
            .get_host_port_ipv4(6379)
            .await
            .expect("Failed to get Redis port");

        let database_url = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            postgres_port
        );
        let redis_uri = format!("redis://localhost:{}", redis_port);

        Self {
            postgres_container,
            redis_container,
            database_url,
            redis_uri,
        }
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn redis_uri(&self) -> &str {
        &self.redis_uri
    }
}

pub async fn get_test_environment() -> &'static TestEnvironment {
    TEST_ENV
        .get_or_init(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(TestEnvironment::new())
            })
        })
}

/// Connects to the shared test Postgres container and applies migrations,
/// returning a pool ready for use. Each call reuses the same underlying
/// database (Postgres testcontainers only expose one DB per container) —
/// callers that need isolation should clean up their own rows/tables.
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

pub async fn get_test_redis() -> redis::Connection {
    let env = get_test_environment().await;

    redis::Client::open(env.redis_uri())
        .expect("Failed to create Redis client")
        .get_connection()
        .expect("Failed to get Redis connection")
}

/// Truncates all tables so the shared test database starts clean for the next test.
pub async fn cleanup_test_db(pool: &sqlx::PgPool) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "TRUNCATE TABLE admins, locations, batches, students, sessions, attendances, \
             devices, device_fingerprints, flags, photo_hashes, short_links, system_configs, \
             webauthn_challenges, webauthn_credentials, webauthn_reenrollment_logs \
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
