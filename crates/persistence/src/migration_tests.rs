//! Tests for the embedded SQLite migrations.
//!
//! These run the real [`crate::MIGRATIONS`] against a fresh in-memory SQLite database to prove
//! that the migration set applies cleanly up, that every (reversible) migration rolls back via
//! its `down.sql`, that re-applying after a full rollback is idempotent, and that the resulting
//! schema matches what `schema.rs` expects. Previously there was no coverage for migration
//! correctness at all.

use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel::{Connection, RunQueryDsl};
use diesel_migrations::MigrationHarness;

use crate::MIGRATIONS;

/// A fresh, isolated in-memory database for each test. Foreign-key enforcement is enabled so
/// that down-migrations which drop/recreate tables surface referential problems.
fn in_memory_connection() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory sqlite db");
    conn.batch_execute("PRAGMA foreign_keys = ON;")
        .expect("enable foreign keys");
    conn
}

/// Returns the number of migrations diesel embedded from the `migrations/` directory.
fn embedded_migration_count() -> usize {
    use diesel::migration::MigrationSource;
    MigrationSource::<diesel::sqlite::Sqlite>::migrations(&MIGRATIONS)
        .expect("enumerate embedded migrations")
        .len()
}

#[test]
fn all_migrations_apply_cleanly_up() {
    let mut conn = in_memory_connection();

    let applied = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("all migrations must apply without error");

    assert_eq!(
        applied.len(),
        embedded_migration_count(),
        "every embedded migration should have been applied on a fresh database"
    );
    assert!(
        !conn
            .has_pending_migration(MIGRATIONS)
            .expect("query pending migrations"),
        "no migrations should remain pending after a full up"
    );
}

#[test]
fn running_migrations_twice_is_idempotent() {
    let mut conn = in_memory_connection();

    let first = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("first run applies all migrations");
    assert_eq!(first.len(), embedded_migration_count());

    // A second run on the same database must be a no-op (nothing pending), not an error.
    let second = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("second run must succeed");
    assert!(
        second.is_empty(),
        "re-running migrations on an up-to-date database should apply nothing, got {} migrations",
        second.len()
    );
}

#[test]
fn every_migration_reverts_and_reapplies() {
    let mut conn = in_memory_connection();

    conn.run_pending_migrations(MIGRATIONS)
        .expect("apply all migrations up");

    // Revert every migration one at a time using each migration's `down.sql`. Each revert must
    // succeed: a `down.sql` that errors unconditionally (e.g. a SQLite-invalid statement) is a
    // broken migration even though production never calls revert. This proves every down
    // direction is well-formed SQL that SQLite accepts in reverse order.
    let total = embedded_migration_count();
    for n in 1..=total {
        conn.revert_last_migration(MIGRATIONS).unwrap_or_else(|e| {
            let still_applied = conn.applied_migrations().map(|m| m.len()).unwrap_or(0);
            panic!("revert {n}/{total} failed ({still_applied} still applied): {e}")
        });
    }

    assert!(
        conn.has_pending_migration(MIGRATIONS)
            .expect("query pending migrations"),
        "all migrations should be pending again after a full revert"
    );
    assert!(
        conn.applied_migrations()
            .expect("query applied migrations")
            .is_empty(),
        "no migrations should remain applied after a full revert"
    );

    // Re-apply everything to prove the up/down/up cycle is consistent.
    let reapplied = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("migrations must re-apply after a full revert");
    assert_eq!(reapplied.len(), total);
}

#[test]
fn final_schema_matches_renamed_columns() {
    // The 2026-06-10 migration renamed `warp_ai_width` -> `ai_width` and
    // `warp_drive_index_width` -> `drive_index_width` on the `windows` table, and `schema.rs`
    // reflects the new names. Verify the migrated schema agrees, which also confirms the
    // migrations ran in order and the ALTER/RENAME chain landed.
    let mut conn = in_memory_connection();
    conn.run_pending_migrations(MIGRATIONS)
        .expect("apply all migrations");

    let columns: Vec<String> = diesel::sql_query("PRAGMA table_info(windows);")
        .load::<TableInfoRow>(&mut conn)
        .expect("read windows table_info")
        .into_iter()
        .map(|row| row.name)
        .collect();

    assert!(
        columns.iter().any(|c| c == "ai_width"),
        "expected renamed column `ai_width` on windows, got: {columns:?}"
    );
    assert!(
        columns.iter().any(|c| c == "drive_index_width"),
        "expected renamed column `drive_index_width` on windows, got: {columns:?}"
    );
    assert!(
        !columns.iter().any(|c| c == "warp_ai_width"),
        "old column `warp_ai_width` should no longer exist after migration"
    );
}

#[test]
fn expected_tables_exist_after_migration() {
    let mut conn = in_memory_connection();
    conn.run_pending_migrations(MIGRATIONS)
        .expect("apply all migrations");

    let tables: Vec<String> = diesel::sql_query(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%';",
    )
    .load::<TableNameRow>(&mut conn)
    .expect("list tables")
    .into_iter()
    .map(|row| row.name)
    .collect();

    // A few representative tables that `schema.rs` declares and the app relies on.
    for expected in ["windows", "tabs", "blocks", "commands", "app"] {
        assert!(
            tables.iter().any(|t| t == expected),
            "expected table `{expected}` to exist after migrations, got: {tables:?}"
        );
    }
}

#[derive(diesel::QueryableByName)]
struct TableInfoRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    name: String,
}

#[derive(diesel::QueryableByName)]
struct TableNameRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    name: String,
}
