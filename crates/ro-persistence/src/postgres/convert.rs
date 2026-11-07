//! Checked integer conversions between Rust values and PostgreSQL columns.

use sqlx::postgres::PgRow;

use super::mapping::get_col;
use crate::RepositoryError;

fn bad(col: &str, value: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Internal(format!("column '{col}' out of range: {value}"))
}

/// Reads an integer column stored wider than its Rust type.
pub(super) fn narrow<T: TryFrom<i64>>(col: &str, wide: i64) -> Result<T, RepositoryError> {
    T::try_from(wide).map_err(|_| bad(col, wide))
}

pub(super) fn big<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i64 = get_col(row, col)?;
    narrow(col, wide)
}

pub(super) fn int<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i32 = get_col(row, col)?;
    narrow(col, i64::from(wide))
}

pub(super) fn small<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i16 = get_col(row, col)?;
    narrow(col, i64::from(wide))
}

/// A value for a `SMALLINT` column; one that doesn't fit is refused, not wrapped.
pub(super) fn to_small(col: &str, value: u16) -> Result<i16, RepositoryError> {
    i16::try_from(value).map_err(|_| bad(col, value))
}

/// A value for an `INT` column.
pub(super) fn to_int(col: &str, value: u32) -> Result<i32, RepositoryError> {
    i32::try_from(value).map_err(|_| bad(col, value))
}

/// A value for a `BIGINT` column.
pub(super) fn to_big(col: &str, value: u64) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|_| bad(col, value))
}
