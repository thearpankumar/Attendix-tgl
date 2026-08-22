//! Verifies the core multi-container-safety guarantee of the recurring
//! session scheduler: two backend replicas racing to claim the same due
//! rule at the same instant must produce exactly one generated session, not
//! two. This is the property the whole `FOR UPDATE SKIP LOCKED` design in
//! `services::recurring_scheduler` exists for — see that module's doc
//! comment for the mechanism.
//!
//! Uses two independent `PgPool`s against the same shared testcontainers
//! Postgres instance, mirroring two separate backend container replicas
//! rather than two tasks sharing one pool.

use chrono::Utc;
use serial_test::file_serial;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

async fn insert_fixture_admin(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO admins (id, username, email, password, role, is_active, created_at) \
         VALUES ($1, $2, $3, 'irrelevant-hash', 'super_admin', true, now())",
    )
    .bind(id)
    .bind(format!("recurring-sched-admin-{id}"))
    .bind(format!("recurring-sched-admin-{id}@example.com"))
    .execute(pool)
    .await
    .expect("insert fixture admin");
    id
}

async fn insert_fixture_location(pool: &sqlx::PgPool, created_by: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO locations (id, name, latitude, longitude, radius_meters, created_by, is_active, created_at) \
         VALUES ($1, 'Recurring Scheduler Test Location', 12.9716, 77.5946, 100.0, $2, true, now())",
    )
    .bind(id)
    .bind(created_by)
    .execute(pool)
    .await
    .expect("insert fixture location");
    id
}

/// Inserts a rule that's already due (`next_run_at` in the past) so the very
/// next claim attempt picks it up.
async fn insert_due_rule(pool: &sqlx::PgPool, location_id: Uuid, created_by: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    let past = Utc::now() - chrono::Duration::seconds(5);
    sqlx::query(
        "INSERT INTO recurring_session_rules \
         (id, location_id, batch_id, description, duration_minutes, run_times_local, timezone, \
          days_of_week, is_active, created_by, created_at, updated_at, locked_short_code, next_run_at) \
         VALUES ($1, $2, NULL, 'race test rule', 30, ARRAY['09:00'::time], 'UTC', \
                 ARRAY[0,1,2,3,4,5,6]::smallint[], true, $3, now(), now(), NULL, $4)",
    )
    .bind(id)
    .bind(location_id)
    .bind(created_by)
    .bind(past)
    .execute(pool)
    .await
    .expect("insert due recurring rule");
    id
}

async fn second_pool_against_same_db() -> sqlx::PgPool {
    let env = crate::test_db::get_test_environment().await;
    PgPoolOptions::new()
        .max_connections(5)
        .connect(env.database_url())
        .await
        .expect("connect second pool to shared test database")
}

#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn concurrent_claims_from_two_replicas_generate_exactly_one_session() {
    // No table-wide cleanup here: this test binary's other files (e.g.
    // exam_session_flow_tests) run concurrently against the same shared
    // testcontainers database and rely on their own previously-inserted
    // rows staying put, so this test isolates itself with fresh UUIDs
    // instead of truncating shared tables like admins/sessions.
    let pool_a = crate::test_db::get_test_database().await;
    let pool_b = second_pool_against_same_db().await;

    let admin_id = insert_fixture_admin(&pool_a).await;
    let location_id = insert_fixture_location(&pool_a, admin_id).await;
    let rule_id = insert_due_rule(&pool_a, location_id, admin_id).await;

    // Two "replicas" racing the same claim query at (as close to) the same
    // instant as this process can manage.
    let (result_a, result_b) = tokio::join!(
        attendance_geotag_backend::services::claim_and_generate_one(&pool_a),
        attendance_geotag_backend::services::claim_and_generate_one(&pool_b),
    );

    let claimed_a = result_a.expect("replica A's claim pass should not error");
    let claimed_b = result_b.expect("replica B's claim pass should not error");

    // Exactly one of the two replicas should have found (and processed) the
    // due rule; SKIP LOCKED makes the other see zero due rows.
    assert_eq!(
        [claimed_a, claimed_b].iter().filter(|c| **c).count(),
        1,
        "expected exactly one replica to claim the due rule (a={claimed_a}, b={claimed_b})"
    );

    let session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE recurring_rule_id = $1")
            .bind(rule_id)
            .fetch_one(&pool_a)
            .await
            .expect("count generated sessions");

    assert_eq!(
        session_count, 1,
        "recurring rule must generate exactly one session across two racing replicas, got {session_count}"
    );

    // The rule itself should reflect a single successful generation: pointed
    // at that one session, failure counters clean, and scheduled forward
    // (not stuck in the past, which would cause it to be claimed again next
    // tick and double-generate).
    let (last_generated_session_id, consecutive_failures, next_run_at): (
        Option<Uuid>,
        i32,
        chrono::DateTime<Utc>,
    ) = sqlx::query_as(
        "SELECT last_generated_session_id, consecutive_failures, next_run_at \
         FROM recurring_session_rules WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_one(&pool_a)
    .await
    .expect("fetch rule after generation");

    assert!(
        next_run_at > Utc::now(),
        "next_run_at must advance into the future"
    );
    assert_eq!(consecutive_failures, 0);
    assert!(last_generated_session_id.is_some());

    let generated_session_id: Uuid =
        sqlx::query_scalar("SELECT id FROM sessions WHERE recurring_rule_id = $1")
            .bind(rule_id)
            .fetch_one(&pool_a)
            .await
            .expect("fetch the generated session id");
    assert_eq!(last_generated_session_id, Some(generated_session_id));

    pool_b.close().await;
}

#[tokio::test]
#[file_serial(recurring_scheduler)]
async fn claim_returns_false_when_nothing_is_due() {
    // `recurring_session_rules` is only ever written to by this file's
    // tests, and `#[file_serial]` runs them one at a time, so a fresh rule
    // scoped to this test (never due) is enough to prove "nothing due"
    // without needing to touch/assume anything about the table's other rows.
    let pool = crate::test_db::get_test_database().await;

    let admin_id = insert_fixture_admin(&pool).await;
    let location_id = insert_fixture_location(&pool, admin_id).await;
    let id = Uuid::new_v4();
    let far_future = Utc::now() + chrono::Duration::days(30);
    sqlx::query(
        "INSERT INTO recurring_session_rules \
         (id, location_id, batch_id, description, duration_minutes, run_times_local, timezone, \
          days_of_week, is_active, created_by, created_at, updated_at, locked_short_code, next_run_at) \
         VALUES ($1, $2, NULL, 'not-due test rule', 30, ARRAY['09:00'::time], 'UTC', \
                 ARRAY[0,1,2,3,4,5,6]::smallint[], true, $3, now(), now(), NULL, $4)",
    )
    .bind(id)
    .bind(location_id)
    .bind(admin_id)
    .bind(far_future)
    .execute(&pool)
    .await
    .expect("insert not-due recurring rule");

    let claimed = attendance_geotag_backend::services::claim_and_generate_one(&pool)
        .await
        .expect("claim pass should not error when nothing is due");
    assert!(!claimed);
}
