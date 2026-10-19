//! Account repository implementation and entity mappings.

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::query;

use ro_contracts::account::{Account, AccountRepository, AccountState, AuthError};
use ro_contracts::ids::AccountId;
use ro_contracts::repository::RepositoryError;

use super::{get_col, map_account_state, map_sex, map_sqlx_error, PostgresRepo};

/// Maps a database row to an `Account` entity using the row extraction helper.
pub(crate) fn row_to_account(row: &PgRow) -> Result<Account, RepositoryError> {
    let account_id: i32 = get_col(row, "account_id")?;
    let username: String = get_col(row, "username")?;
    let password_hash: String = get_col(row, "password_hash")?;
    let sex: i16 = get_col(row, "sex")?;
    let email: String = get_col(row, "email")?;
    let group_id: i32 = get_col(row, "group_id")?;
    let state: i16 = get_col(row, "state")?;
    let unban_time: Option<i64> = get_col(row, "unban_time")?;
    let expiration_time: Option<i64> = get_col(row, "expiration_time")?;

    Ok(Account {
        id: AccountId(account_id as u32),
        username,
        password_hash,
        sex: map_sex(sex),
        email,
        group_id,
        state: map_account_state(state),
        unban_time: unban_time.map(|t| t as u64),
        expiration_time: expiration_time.map(|t| t as u64),
    })
}

#[async_trait]
impl AccountRepository for PostgresRepo {
    async fn authenticate(&self, username: &str, password: &str) -> Result<Account, AuthError> {
        let row = query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        let row = match row {
            Some(r) => r,
            None => return Err(AuthError::UnregisteredId),
        };

        let account = row_to_account(&row)?;

        match account.state {
            AccountState::Banned | AccountState::Locked => return Err(AuthError::AccountBanned),
            AccountState::Expired => return Err(AuthError::AccountExpired),
            AccountState::Active => {}
        }

        if account.password_hash != password {
            return Err(AuthError::IncorrectPassword);
        }

        Ok(account)
    }

    async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>, AuthError> {
        let row = query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE account_id = $1
            "#,
        )
        .bind(account_id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        match row {
            Some(r) => Ok(Some(row_to_account(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<Account>, AuthError> {
        let row = query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        match row {
            Some(r) => Ok(Some(row_to_account(&r)?)),
            None => Ok(None),
        }
    }
}
