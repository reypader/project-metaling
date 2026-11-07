//! Integration tests for `PostgresRepo` (PERSIST-AUTH, PERSIST-CHAR, PERSIST-SCHEMA, PER-INV, PER-EQ,
//! PER-TRAIT, PER-EFF, PER-SKILL). Needs the database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

mod auth;
mod char;
mod inventory;
mod progress;
mod schema;
