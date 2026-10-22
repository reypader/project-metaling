//! Resolves rAthena's NPC manifest (`npc/re/scripts_main.conf`) into the ordered file list
//! (spec TOOL-NPC-01; `reference/catalog/db/npc_files.md` §1).

use std::path::Path;

/// Deepest `import:` nesting followed; the default manifest uses two levels.
const MAX_IMPORT_DEPTH: usize = 16;

/// Ordered NPC file paths (relative to the rAthena root, exactly as written), plus problems
/// that rAthena reports and skips.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub files: Vec<String>,
    pub warnings: Vec<String>,
}

/// Starts at `entry` (relative to `root`) and follows `npc:`, `delnpc:` and `import:`.
pub fn resolve(root: &Path, entry: &str) -> Manifest {
    let mut manifest = Manifest::default();
    read_conf(root, entry, 0, &mut manifest);
    manifest
}

fn read_conf(root: &Path, conf: &str, depth: usize, out: &mut Manifest) {
    if depth > MAX_IMPORT_DEPTH {
        out.warnings.push(format!(
            "{conf}: import nesting deeper than {MAX_IMPORT_DEPTH}"
        ));
        return;
    }
    let text = match std::fs::read(root.join(conf)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(err) => {
            out.warnings.push(format!("{conf}: {err}"));
            return;
        }
    };
    for raw in text.lines() {
        if raw.starts_with("//") {
            continue;
        }
        let line = raw.find("//").map_or(raw, |i| &raw[..i]);
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim_start_matches(' ');
        let value = value
            .find(['\t', '\r', '\n'])
            .map_or(value, |i| &value[..i])
            .trim_end_matches(' ');
        match key.to_ascii_lowercase().as_str() {
            "npc" if value.eq_ignore_ascii_case("clear") => out.files.clear(),
            "npc" => {
                if out.files.iter().any(|f| f == value) {
                    continue;
                }
                if root.join(value).is_file() {
                    out.files.push(value.to_string());
                } else {
                    out.warnings
                        .push(format!("{conf}: npc file '{value}' not found"));
                }
            }
            "delnpc" if value.eq_ignore_ascii_case("all") => out.files.clear(),
            "delnpc" => out.files.retain(|f| f != value),
            "import" => read_conf(root, value, depth + 1, out),
            _ => {}
        }
    }
}
