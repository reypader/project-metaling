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
