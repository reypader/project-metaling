//! Shared plumbing for the per-map YAML datasets (`Header` + `Body` files from `tools/npc-import`):
//! header check, optional-file reading, and YAML 1.2 output.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::DbLoadError;

/// Which file type and version a dataset reads and writes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Schema {
    pub db_type: &'static str,
    pub version: u32,
}

impl Schema {
    /// Parses a file and checks its header. `path` is only a label for errors.
    pub(crate) fn parse<T: DeserializeOwned>(
        self,
        path: &str,
        text: &str,
    ) -> Result<Vec<T>, DbLoadError> {
        let file: DbFile<T> = serde_saphyr::from_str(text).map_err(|e| DbLoadError::Parse {
            path: path.to_string(),
            line: e.location().and_then(|l| usize::try_from(l.line()).ok()),
            message: e.to_string(),
        })?;
        let header = &file.header;
        if header.r#type != self.db_type || header.version != self.version {
            return Err(DbLoadError::Schema {
                path: path.to_string(),
                message: format!(
                    "expected header {} version {}, got {} version {}",
                    self.db_type, self.version, header.r#type, header.version
                ),
            });
        }
        Ok(file.body)
    }

    /// Reads and parses one file. A missing file is `Ok(None)`.
    pub(crate) fn load<T: DeserializeOwned>(
        self,
        path: &Path,
    ) -> Result<Option<Vec<T>>, DbLoadError> {
        let label = path.display().to_string();
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(DbLoadError::Io {
                    path: label,
                    message: err.to_string(),
                })
            }
        };
        self.parse(&label, &text).map(Some)
    }

    /// Serializes with this header (no leading comments; the importer adds those).
    pub(crate) fn to_yaml<T: Serialize>(self, body: &[T]) -> Result<String, DbLoadError> {
        let file = DbFileRef {
            header: Header {
                r#type: self.db_type.to_string(),
                version: self.version,
            },
            body,
        };
        // YAML 1.2 rules: the loader is YAML 1.2, so keys like `Y` need no 1.1-boolean quoting.
        let mut options = serde_saphyr::SerializerOptions::default();
        options.yaml_12 = true;
        serde_saphyr::to_string_with_options(&file, options).map_err(|e| DbLoadError::Schema {
            path: String::new(),
            message: e.to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Header {
    r#type: String,
    version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct DbFile<T> {
    header: Header,
    #[serde(default = "Vec::new")]
    body: Vec<T>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DbFileRef<'a, T> {
    header: Header,
    body: &'a [T],
}

/// `skip_serializing_if` helper for flags that default to `false`.
pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}
