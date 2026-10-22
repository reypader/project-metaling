//! Classifies a header record and reads its location, name, sprite token and spans
//! (spec TOOL-NPC-03/04; `reference/catalog/db/npc_files.md` §3-4).

use ro_db::NpcState;

use crate::lex::Record;

/// rAthena `NPC_NAME_LENGTH`.
pub const NPC_NAME_LENGTH: usize = 50;
/// rAthena reads the map name with `%15[^,]`.
const MAP_NAME_MAX: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub map: String,
    pub x: i16,
    pub y: i16,
    pub dir: i16,
}

/// Where an NPC header puts it: on a map, or floating (`-`, not on any map).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Map(Location),
    Floating,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Warp {
        loc: Location,
        warp2: bool,
        state: NpcState,
        span: (i16, i16),
        dest_map: String,
        dest: (i16, i16),
    },
    Script {
        place: Place,
        state: NpcState,
        sprite: String,
        /// OnTouch half-extents when both are given and non-negative.
        trigger: Option<(i16, i16)>,
    },
    Shop {
        place: Place,
        sprite: String,
    },
    Duplicate {
        place: Place,
        src: String,
        /// Raw `w4`: its meaning depends on the source's kind.
        w4: String,
    },
    /// Not an NPC this tool converts (`function`, `monster`, `mapflag`, ...).
    Skip(SkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkipReason {
    Function,
    Monster,
    BossMonster,
    Mapflag,
    UnknownKeyword,
}

/// Why a header that looked like an NPC could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderError(pub String);

const SHOP_KEYWORDS: [&str; 5] = ["shop", "cashshop", "itemshop", "pointshop", "marketshop"];

pub fn parse(rec: &Record) -> Result<Header, HeaderError> {
    let w2 = rec.w2.as_str();
    let lower = w2.to_ascii_lowercase();
    if lower.starts_with("warp") {
        return parse_warp(rec, &lower);
    }
    if SHOP_KEYWORDS.contains(&lower.as_str()) {
        return Ok(Header::Shop {
            place: parse_place(&rec.w1, true)?,
            sprite: sprite_token(&rec.w4).to_string(),
        });
    }
    if lower.starts_with("script") {
        if rec.w1.eq_ignore_ascii_case("function") {
            return Ok(Header::Skip(SkipReason::Function));
        }
        return Ok(Header::Script {
            place: parse_place(&rec.w1, true)?,
            state: parse_state(&lower),
            sprite: sprite_token(&rec.w4).to_string(),
            trigger: after_first_comma_pair(&rec.w4).filter(|&(x, y)| x >= 0 && y >= 0),
        });
    }
    if lower.starts_with("duplicate(") {
        let src = w2
            .get("duplicate(".len()..)
            .and_then(|rest| rest.rfind(')').map(|end| &rest[..end]))
            .ok_or_else(|| HeaderError(format!("bad duplicate source in '{w2}'")))?;
        return Ok(Header::Duplicate {
            place: parse_place(&rec.w1, true)?,
            src: src.to_string(),
            w4: rec.w4.clone(),
        });
    }
    Ok(Header::Skip(match lower.as_str() {
        "monster" => SkipReason::Monster,
        "boss_monster" => SkipReason::BossMonster,
        "mapflag" => SkipReason::Mapflag,
        _ => SkipReason::UnknownKeyword,
    }))
}

fn parse_warp(rec: &Record, lower: &str) -> Result<Header, HeaderError> {
    let loc = parse_location(&rec.w1, false)?;
    let mut parts = rec.w4.splitn(5, ',');
    let mut num = |what: &str| {
        parts
            .next()
            .and_then(leading_i16)
            .ok_or_else(|| HeaderError(format!("warp '{}': bad {what}", rec.w3)))
    };
    let span = (num("xs")?, num("ys")?);
    let dest_map = parts
        .next()
        .filter(|m| !m.is_empty() && m.len() <= MAP_NAME_MAX)
        .ok_or_else(|| HeaderError(format!("warp '{}': bad destination map", rec.w3)))?
        .to_string();
    let mut num = |what: &str| {
        parts
            .next()
            .and_then(leading_i16)
            .ok_or_else(|| HeaderError(format!("warp '{}': bad {what}", rec.w3)))
    };
    let dest = (num("dest x")?, num("dest y")?);
    Ok(Header::Warp {
        loc,
        warp2: lower.starts_with("warp2"),
        state: parse_state(lower),
        span,
        dest_map,
        dest,
    })
}

/// `(CLOAKED)`, `(HIDDEN)`, `(DISABLED)` after the keyword; anything else is visible.
fn parse_state(lower_w2: &str) -> NpcState {
    let Some(open) = lower_w2.find('(') else {
        return NpcState::Normal;
    };
    match lower_w2[open..].trim_end() {
        "(cloaked)" => NpcState::Cloaked,
        "(hidden)" => NpcState::Hidden,
        "(disabled)" => NpcState::Disabled,
        _ => NpcState::Normal,
    }
}

fn parse_place(w1: &str, with_dir: bool) -> Result<Place, HeaderError> {
    if w1 == "-" {
        return Ok(Place::Floating);
    }
    parse_location(w1, with_dir).map(Place::Map)
}

/// `map,x,y[,dir]` like `sscanf("%15[^,],%6hd,%6hd,%4hd")`: numbers may have leading spaces,
/// extra trailing fields are ignored. Warps don't read `dir` (it is 0).
fn parse_location(w1: &str, with_dir: bool) -> Result<Location, HeaderError> {
    let bad = || HeaderError(format!("bad location '{w1}'"));
    let mut parts = w1.split(',');
    let map = parts
        .next()
        .filter(|m| !m.is_empty() && m.len() <= MAP_NAME_MAX)
        .ok_or_else(bad)?;
    let x = parts.next().and_then(leading_i16).ok_or_else(bad)?;
    let y = parts.next().and_then(leading_i16).ok_or_else(bad)?;
    let dir = if with_dir {
        parts.next().and_then(leading_i16).ok_or_else(bad)?
    } else {
        0
    };
    Ok(Location {
        map: map.to_string(),
        x,
        y,
        dir,
    })
}

/// `%hd`: optional leading whitespace and sign, then at least one digit; the rest is ignored.
pub fn leading_i16(s: &str) -> Option<i16> {
    let s = s.trim_start();
    let digits_start = usize::from(s.starts_with(['-', '+']));
    let len = s[digits_start..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    if len == 0 {
        return None;
    }
    s[..digits_start + len].parse().ok()
}

/// `%*[^,],%6hd,%6hd`: two numbers after the first comma (needs a non-empty first token).
pub fn after_first_comma_pair(w4: &str) -> Option<(i16, i16)> {
    let (first, rest) = w4.split_once(',')?;
    if first.is_empty() {
        return None;
    }
    let mut parts = rest.split(',');
    let a = parts.next().and_then(leading_i16)?;
    let b = parts.next().and_then(leading_i16)?;
    Some((a, b))
}

/// `%6hd,%6hd` at the start of `w4` (warp duplicates).
pub fn leading_pair(w4: &str) -> Option<(i16, i16)> {
    let mut parts = w4.split(',');
    let a = parts.next().and_then(leading_i16)?;
    let b = parts.next().and_then(leading_i16)?;
    Some((a, b))
}

/// First token of `w4`, ending at whitespace, `/` or `,` (rAthena `npc_parseview`).
pub fn sprite_token(w4: &str) -> &str {
    let end = w4
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == ',')
        .unwrap_or(w4.len());
    &w4[..end]
}

/// `(display, unique)` from `w3`: split at the first `::`; without it both are the full
/// string. Each is cut to `NPC_NAME_LENGTH` bytes (on a char boundary). The unique name is
/// `None` when rAthena would generate one (empty, or still containing `::`).
pub fn parse_name(w3: &str) -> (String, Option<String>) {
    let (display, unique) = match w3.split_once("::") {
        Some((d, u)) => (d, u),
        None => (w3, w3),
    };
    let unique = truncate(unique);
    let unique = (!unique.is_empty() && !unique.contains("::")).then_some(unique);
    (truncate(display), unique)
}

fn truncate(s: &str) -> String {
    if s.len() <= NPC_NAME_LENGTH {
        return s.to_string();
    }
    let mut end = NPC_NAME_LENGTH;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}
