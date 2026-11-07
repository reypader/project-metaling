//! Schema setup, migrations and connection errors (PERSIST-SCHEMA-001, 002, PER-EQ-02, PER-TRAIT-01).

use ro_core::AccountId;
use ro_persistence::{CharacterRepository, InventoryRepository, PostgresRepo, RepositoryError};

use crate::common::*;

#[tokio::test]
async fn test_persist_schema_001_schema_initialization_idempotency() {
    let repo = get_repo().await;

    // Execute schema initialization consecutively to verify DDL idempotency
    repo.init_schema().await.unwrap();
    repo.init_schema().await.unwrap();

    // Verify accounts table exists and has proper columns
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert!(count >= 1);

    // Verify characters table exists
    let char_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert!(char_count >= 0);
}

#[tokio::test]
async fn test_persist_schema_002_connection_error_handling() {
    let invalid_url = "postgres://ragnarok:ragnarok@localhost:5433/ragnarok";
    let result = PostgresRepo::connect(invalid_url).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, RepositoryError::Unavailable(_)));
}

/// PER-EQ-02: the schema pass is idempotent on a fresh database and on one that still has the
/// old `weapon_view` column (renamed to `weapon_type`, values kept; `robe_view` added). Runs in
/// its own Postgres schema so the shared tables aren't touched.
#[tokio::test]
async fn per_eq_02_schema_migrates_look_columns() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::str::FromStr;

    let main = get_repo().await;
    let schema = format!("per_eq_02_{}", next_account_id().0);
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(main.pool())
        .await
        .expect("schema created");
    let options = PgConnectOptions::from_str(&test_db_url())
        .expect("db url")
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connects");
    let repo = PostgresRepo::new(pool);

    // Fresh database, twice.
    repo.init_schema().await.expect("fresh schema");
    repo.init_schema().await.expect("idempotent");
    let cols = character_columns(&main, &schema).await;
    assert!(cols.iter().any(|c| c == "weapon_type"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "robe_view"), "{cols:?}");
    assert!(!cols.iter().any(|c| c == "weapon_view"), "{cols:?}");

    // Turn it back into the old layout, with a character wearing something.
    sqlx::raw_sql(
        "ALTER TABLE characters RENAME COLUMN weapon_type TO weapon_view; \
         ALTER TABLE characters DROP COLUMN robe_view; \
         INSERT INTO accounts (account_id, username, password_hash) VALUES (2000000, 'old', 'x'); \
         INSERT INTO characters (account_id, slot, name, sex, weapon_view, head_top_view) \
         VALUES (2000000, 0, 'Old', 1, 7, 16);",
    )
    .execute(repo.pool())
    .await
    .expect("old layout");

    repo.init_schema().await.expect("migrates");
    repo.init_schema()
        .await
        .expect("idempotent after migration");
    let cols = character_columns(&main, &schema).await;
    assert!(cols.iter().any(|c| c == "weapon_type"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "robe_view"), "{cols:?}");
    assert!(!cols.iter().any(|c| c == "weapon_view"), "{cols:?}");
    let old = repo
        .list_by_account(AccountId(2_000_000))
        .await
        .expect("lists");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].looks.weapon_type, 7);
    assert_eq!(old[0].looks.head_top, 16);
    assert_eq!(old[0].looks.robe, 0);

    repo.pool().close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(main.pool())
        .await
        .expect("schema dropped");
}

/// PER-TRAIT-01: a database from before traits gains the eight columns, defaulting to 0, and
/// the pass stays idempotent. Runs in its own Postgres schema, as PER-EQ-02.
#[tokio::test]
async fn per_trait_01_schema_adds_trait_columns() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::str::FromStr;

    const TRAIT_COLUMNS: [&str; 8] = [
        "pow",
        "sta",
        "wis",
        "spl",
        "con",
        "crt",
        "trait_point",
        "ap",
    ];
    let main = get_repo().await;
    let schema = format!("per_trait_01_{}", next_account_id().0);
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(main.pool())
        .await
        .expect("schema created");
    let options = PgConnectOptions::from_str(&test_db_url())
        .expect("db url")
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connects");
    let repo = PostgresRepo::new(pool);

    repo.init_schema().await.expect("fresh schema");
    sqlx::raw_sql(&format!(
        "ALTER TABLE characters DROP COLUMN {}; \
         INSERT INTO accounts (account_id, username, password_hash) VALUES (2000000, 'old', 'x'); \
         INSERT INTO characters (account_id, slot, name, sex) VALUES (2000000, 0, 'Old', 1);",
        TRAIT_COLUMNS.join(", DROP COLUMN ")
    ))
    .execute(repo.pool())
    .await
    .expect("old layout");

    repo.init_schema().await.expect("migrates");
    repo.init_schema()
        .await
        .expect("idempotent after migration");
    let cols = character_columns(&main, &schema).await;
    for col in TRAIT_COLUMNS {
        assert!(cols.iter().any(|c| c == col), "{col}: {cols:?}");
    }
    let old = repo
        .list_by_account(AccountId(2_000_000))
        .await
        .expect("lists");
    let state = repo
        .load_play_state(old[0].char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(
        (
            state.status.traits,
            state.status.trait_point,
            state.status.ap
        ),
        ([0; 6], 0, 0)
    );

    repo.pool().close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(main.pool())
        .await
        .expect("schema dropped");
}
