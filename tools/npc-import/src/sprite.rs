//! Sprite token resolution (spec TOOL-NPC-05; `reference/catalog/db/npc_files.md` §5).

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

/// `JT_INVISIBLE`: what rAthena uses for an unknown sprite. Never sent to clients.
pub const INVISIBLE: i16 = 32767;
/// `JT_WARPNPC`: the class of every warp.
pub const WARPNPC: i16 = 45;

/// Name and mob tables needed to turn a sprite token into a class id.
#[derive(Debug, Default)]
pub struct SpriteTable {
    /// `JT_` constants, lowercase, prefix stripped.
    constants: HashMap<String, i64>,
    /// Mob AegisName (lowercase) -> id.
    mob_by_name: HashMap<String, i64>,
    mob_ids: HashSet<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    pub id: i16,
    pub is_mob: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MobDb {
    #[serde(default)]
    body: Vec<MobEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MobEntry {
    id: i64,
    aegis_name: String,
}

impl SpriteTable {
    /// Builds the table from `src/map/npc.hpp` (`enum e_job_types`) and `mob_db.yml` texts.
    pub fn from_sources(npc_hpp: &str, mob_dbs: &[&str]) -> anyhow::Result<Self> {
        let mut table = Self {
            constants: parse_job_enum(npc_hpp)?,
            ..Self::default()
        };
        for text in mob_dbs {
            // rAthena's mob_db repeats a few keys (e.g. a RaceGroups entry); its loader keeps
            // the last one.
            let mut options = serde_saphyr::Options::default();
            options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::LastWins;
            let db: MobDb = serde_saphyr::from_str_with_options(text, options)?;
            for mob in db.body {
                table
                    .mob_by_name
                    .insert(mob.aegis_name.to_ascii_lowercase(), mob.id);
                table.mob_ids.insert(mob.id);
            }
        }
        Ok(table)
    }

    /// Resolves like `npc_parseview`: C number (base 0), then `JT_` constant, then mob
    /// AegisName, else `None` (the caller uses `INVISIBLE` and warns). Ids outside `i16` are
    /// also `None`.
    pub fn resolve(&self, token: &str) -> Option<Sprite> {
        let lower = token.to_ascii_lowercase();
        let id = parse_c_long(token)
            .or_else(|| self.constants.get(&lower).copied())
            .or_else(|| self.mob_by_name.get(&lower).copied())?;
        Some(Sprite {
            id: i16::try_from(id).ok()?,
            is_mob: self.mob_ids.contains(&id),
        })
    }
}

/// `strtol(token, &end, 0)` that must consume the whole token. An empty token is 0.
fn parse_c_long(token: &str) -> Option<i64> {
    if token.is_empty() {
        return Some(0);
    }
    let (neg, body) = match token.as_bytes()[0] {
        b'-' => (true, &token[1..]),
        b'+' => (false, &token[1..]),
        _ => (false, token),
    };
    let value = if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()?
    } else if body.len() > 1 && body.starts_with('0') {
        i64::from_str_radix(&body[1..], 8).ok()?
    } else if !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit()) {
        body.parse().ok()?
    } else {
        return None;
    };
    Some(if neg { -value } else { value })
}

/// `enum e_job_types { ... }`: `= N` resets the counter, otherwise previous + 1. Only `JT_`
/// members are script constants.
fn parse_job_enum(npc_hpp: &str) -> anyhow::Result<HashMap<String, i64>> {
    let start = npc_hpp
        .find("enum e_job_types")
        .ok_or_else(|| anyhow::anyhow!("enum e_job_types not found in npc.hpp"))?;
    let body_start = npc_hpp[start..]
        .find('{')
        .map(|i| start + i + 1)
        .ok_or_else(|| anyhow::anyhow!("enum e_job_types has no body"))?;
    let body_end = npc_hpp[body_start..]
        .find('}')
        .map(|i| body_start + i)
        .ok_or_else(|| anyhow::anyhow!("enum e_job_types is not closed"))?;
    let body = strip_c_comments(&npc_hpp[body_start..body_end]);

    let mut out = HashMap::new();
    let mut next = 0i64;
    for item in body.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, value) = match item.split_once('=') {
            Some((name, value)) => {
                let value = value.trim();
                let parsed = parse_c_long(value)
                    .ok_or_else(|| anyhow::anyhow!("enum value '{value}' is not a number"))?;
                (name.trim(), parsed)
            }
            None => (item, next),
        };
        next = value + 1;
        if let Some(stripped) = name.strip_prefix("JT_") {
            out.insert(stripped.to_ascii_lowercase(), value);
        }
    }
    Ok(out)
}

fn strip_c_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let line = rest.find("//");
        let block = rest.find("/*");
        let cut = match (line, block) {
            (None, None) => {
                out.push_str(rest);
                return out;
            }
            (Some(l), Some(b)) => l.min(b),
            (Some(l), None) => l,
            (None, Some(b)) => b,
        };
        out.push_str(&rest[..cut]);
        rest = if rest[cut..].starts_with("//") {
            rest[cut..].find('\n').map_or("", |i| &rest[cut + i..])
        } else {
            rest[cut + 2..]
                .find("*/")
                .map_or("", |i| &rest[cut + 2 + i + 2..])
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_npc_05_c_numbers() {
        assert_eq!(parse_c_long("117"), Some(117));
        assert_eq!(parse_c_long("-1"), Some(-1));
        assert_eq!(parse_c_long("0x2D"), Some(45));
        assert_eq!(parse_c_long("010"), Some(8));
        assert_eq!(parse_c_long(""), Some(0));
        assert_eq!(parse_c_long("4_F_KAFRA1"), None);
        assert_eq!(parse_c_long("0"), Some(0));
    }

    #[test]
    fn tool_npc_05_enum_counter_and_resets() {
        let hpp = "enum e_job_types\n{\n\tNPC_RANGE1_START = 44,\n\tJT_WARPNPC,\n\tJT_1_ETC_01, // c\n\t/* x, y */\n\tJT_HIDDEN_WARP_NPC = 139,\n\tJT_NEXT,\n\tJT_FAKENPC = -1\n};";
        let t = parse_job_enum(hpp).expect("parses");
        assert_eq!(t.get("warpnpc"), Some(&45));
        assert_eq!(t.get("1_etc_01"), Some(&46));
        assert_eq!(t.get("hidden_warp_npc"), Some(&139));
        assert_eq!(t.get("next"), Some(&140));
        assert_eq!(t.get("fakenpc"), Some(&-1));
        assert!(!t.contains_key("npc_range1_start"));
    }
}
