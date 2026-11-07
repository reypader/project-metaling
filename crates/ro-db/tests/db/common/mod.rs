//! Helpers shared by the test modules.

use ro_db::DbLoadError;

pub fn names_path(err: &DbLoadError) -> bool {
    match err {
        DbLoadError::Io { path, .. }
        | DbLoadError::Parse { path, .. }
        | DbLoadError::Schema { path, .. }
        | DbLoadError::DuplicateId { path, .. } => path == "bad.yml",
    }
}
