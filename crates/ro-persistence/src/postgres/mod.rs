//! PostgreSQL implementation of repository contracts using `sqlx`.

pub mod account;
pub mod character;
pub mod inventory;
pub mod pool;

pub use pool::PostgresRepo;

use crate::RepositoryError;
use ro_core::{AccountState, Sex};
use sqlx::decode::Decode;
use sqlx::postgres::{PgRow, Postgres};
use sqlx::types::Type;
use sqlx::{Error as SqlxError, Row};

/// Extracts a column from a PostgreSQL row with detailed error reporting on failure.
pub(crate) fn get_col<'r, T>(row: &'r PgRow, col: &str) -> Result<T, RepositoryError>
where
    T: Decode<'r, Postgres> + Type<Postgres>,
{
    row.try_get(col)
        .map_err(|e| RepositoryError::Internal(format!("failed to read column '{col}': {e}")))
}

/// Maps an integer status code to the corresponding `AccountState`.
pub(crate) fn map_account_state(val: i16) -> AccountState {
    match val {
        0 => AccountState::Active,
        1 => AccountState::Banned,
        2 => AccountState::Expired,
        3 => AccountState::Locked,
        _ => AccountState::Active,
    }
}

/// Converts an `AccountState` enum variant to its database integer representation.
pub(crate) fn account_state_to_i16(state: AccountState) -> i16 {
    match state {
        AccountState::Active => 0,
        AccountState::Banned => 1,
        AccountState::Expired => 2,
        AccountState::Locked => 3,
    }
}

/// Maps an integer sex code to the corresponding `Sex` enum variant.
pub(crate) fn map_sex(val: i16) -> Sex {
    match val {
        0 => Sex::Female,
        1 => Sex::Male,
        2 => Sex::Server,
        _ => Sex::Male,
    }
}

/// Converts a `Sex` enum variant to its database integer representation.
pub(crate) fn sex_to_i16(sex: Sex) -> i16 {
    match sex {
        Sex::Female => 0,
        Sex::Male => 1,
        Sex::Server => 2,
    }
}

/// SQLSTATE classes a retry can fix: connection exceptions (08), transaction rollbacks such as
/// serialization failures and deadlocks (40), insufficient resources (53), and operator
/// intervention such as a server shutdown or restart (57).
pub(crate) fn is_transient_sqlstate(code: &str) -> bool {
    matches!(code.get(..2), Some("08" | "40" | "53" | "57"))
}

/// Translates SQLx error variants into standard `RepositoryError` instances. Errors a retry can
/// fix are `Unavailable`.
pub(crate) fn map_sqlx_error(err: SqlxError) -> RepositoryError {
    match err {
        SqlxError::RowNotFound => RepositoryError::NotFound,
        SqlxError::PoolTimedOut
        | SqlxError::PoolClosed
        | SqlxError::WorkerCrashed
        | SqlxError::Io(_) => RepositoryError::Unavailable(err.to_string()),
        SqlxError::Database(db_err) => {
            if db_err
                .code()
                .is_some_and(|code| is_transient_sqlstate(&code))
            {
                RepositoryError::Unavailable(db_err.message().to_string())
            } else if db_err.is_unique_violation() {
                RepositoryError::Conflict(db_err.message().to_string())
            } else {
                RepositoryError::Internal(db_err.message().to_string())
            }
        }
        other => RepositoryError::Internal(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_state() {
        assert_eq!(map_account_state(0), AccountState::Active);
        assert_eq!(map_account_state(1), AccountState::Banned);
        assert_eq!(map_account_state(2), AccountState::Expired);
        assert_eq!(map_account_state(3), AccountState::Locked);
        assert_eq!(map_account_state(99), AccountState::Active);
    }

    #[test]
    fn test_sex_conversions() {
        assert_eq!(map_sex(0), Sex::Female);
        assert_eq!(map_sex(1), Sex::Male);
        assert_eq!(map_sex(2), Sex::Server);
        assert_eq!(map_sex(99), Sex::Male);

        assert_eq!(sex_to_i16(Sex::Female), 0);
        assert_eq!(sex_to_i16(Sex::Male), 1);
        assert_eq!(sex_to_i16(Sex::Server), 2);
    }

    #[test]
    fn test_map_sqlx_error() {
        assert_eq!(
            map_sqlx_error(SqlxError::RowNotFound),
            RepositoryError::NotFound
        );
        assert_eq!(
            map_sqlx_error(SqlxError::PoolTimedOut),
            RepositoryError::Unavailable(SqlxError::PoolTimedOut.to_string())
        );
    }

    /// Errors a retry can fix are `Unavailable` (the map-server writer retries only those).
    #[test]
    fn transient_errors_are_unavailable() {
        for code in ["08006", "40001", "40P01", "53300", "57P01", "57P03"] {
            assert!(is_transient_sqlstate(code), "{code}");
        }
        for code in ["23505", "22003", "42P01", "XX000"] {
            assert!(!is_transient_sqlstate(code), "{code}");
        }
        assert!(matches!(
            map_sqlx_error(SqlxError::PoolClosed),
            RepositoryError::Unavailable(_)
        ));
        assert!(matches!(
            map_sqlx_error(SqlxError::WorkerCrashed),
            RepositoryError::Unavailable(_)
        ));
    }
}
