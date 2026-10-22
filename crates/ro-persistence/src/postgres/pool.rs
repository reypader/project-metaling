//! Connection pool management, schema migrations, and test fixture seeding.

use sqlx::{query, raw_sql, PgPool};

use crate::RepositoryError;
use ro_core::Account;

use super::{account_state_to_i16, map_sqlx_error, sex_to_i16};

/// Authoritative PostgreSQL repository backed by a connection pool (`sqlx::PgPool`).
#[derive(Debug, Clone)]
pub struct PostgresRepo {
    pub(crate) pool: PgPool,
}

impl PostgresRepo {
    /// Wraps an existing PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Connects to a PostgreSQL database using the provided connection string.
    pub async fn connect(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        Ok(Self { pool })
    }

    /// Returns a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Constructs a repository instance from an existing connection pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self::new(pool)
    }

    /// Executes baseline schema initialization (`01_schema.sql`).
    pub async fn init_schema(&self) -> Result<(), RepositoryError> {
        let schema_sql = include_str!("../../sql/01_schema.sql");
        raw_sql(schema_sql)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Resets accounts and character tables by truncating them with identity restart.
    pub async fn reset_tables(&self) -> Result<(), RepositoryError> {
        raw_sql("TRUNCATE TABLE characters, accounts RESTART IDENTITY CASCADE;")
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Seeds baseline development data (`02_seed.sql`).
    pub async fn seed_test_data(&self) -> Result<(), RepositoryError> {
        let seed_sql = include_str!("../../sql/02_seed.sql");
        raw_sql(seed_sql)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Inserts or updates an account directly in the database.
    pub async fn insert_account(&self, account: &Account) -> Result<(), RepositoryError> {
        query(
            r#"
            INSERT INTO accounts (
                account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (account_id) DO UPDATE SET
                username = EXCLUDED.username,
                password_hash = EXCLUDED.password_hash,
                sex = EXCLUDED.sex,
                email = EXCLUDED.email,
                group_id = EXCLUDED.group_id,
                state = EXCLUDED.state,
                unban_time = EXCLUDED.unban_time,
                expiration_time = EXCLUDED.expiration_time
            "#,
        )
        .bind(account.id.0 as i32)
        .bind(&account.username)
        .bind(&account.password_hash)
        .bind(sex_to_i16(account.sex))
        .bind(&account.email)
        .bind(account.group_id)
        .bind(account_state_to_i16(account.state))
        .bind(account.unban_time.map(|t| t as i64))
        .bind(account.expiration_time.map(|t| t as i64))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
