//! Classifies a header record and reads its location, name, sprite token and spans
//! (spec TOOL-NPC-03/04; `reference/catalog/db/npc_files.md` §3-4), or the fields of a mob spawn
//! line (spec TOOL-MOB-01..04; `reference/catalog/mechanics/mobs.md` §1.1, §1.6).

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
    /// A `monster` / `boss_monster` spawn line.
    Mob(MobHeader),
    /// Not something this tool converts (`function`, `mapflag`, ...).
    Skip(SkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkipReason {
    Function,
    Mapflag,
    UnknownKeyword,
}

/// rAthena `NAME_LENGTH - 1`: a mob name is read with `%23[^,]`.
pub const MOB_NAME_MAX: usize = 23;

/// A spawn line's fields as written, before validation and name/id resolution. Absent numbers
/// in `w1` are 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobHeader {
    pub map: String,
    pub x: i16,
    pub y: i16,
    /// Raw `(xs, ys)`.
    pub span: (i16, i16),
    /// The keyword was `boss_monster`.
    pub boss: bool,
    /// The name part of `w3`, cut to `MOB_NAME_MAX` bytes.
    pub name: String,
    pub name_truncated: bool,
    /// The level part of `w3`; `None` when absent or not a number.
    pub level: Option<i64>,
    /// Mob id or AegisName token, trimmed.
    pub mob: String,
    pub amount: i64,
    pub delay1: Option<i64>,
    pub delay2: Option<i64>,
    /// Raw event field, quotes not yet stripped.
    pub event: Option<String>,
    pub size: Option<i64>,
    pub ai: Option<i64>,
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
    if lower == "monster" || lower == "boss_monster" {
        return parse_mob(rec, lower == "boss_monster").map(Header::Mob);
    }
    Ok(Header::Skip(match lower.as_str() {
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

/// Reads a spawn line like rAthena's scanf formats: `w1` `map{,x,y{,xs,ys}}`, `w3`
/// `name{,level}`, `w4` `id,amount{,delay1{,delay2{,event{,size{,ai}}}}}`. Each conversion stops
/// the rest of its field list when it fails.
fn parse_mob(rec: &Record, boss: bool) -> Result<MobHeader, HeaderError> {
    let mut w1 = rec.w1.split(',');
    let map = w1
        .next()
        .filter(|m| !m.is_empty() && m.len() <= MAP_NAME_MAX)
        .ok_or_else(|| HeaderError(format!("bad location '{}'", rec.w1)))?;
    let mut coords = [0i16; 4];
    for (slot, value) in coords.iter_mut().zip(scan_ints(w1)) {
        *slot =
            i16::try_from(value).map_err(|_| HeaderError(format!("bad location '{}'", rec.w1)))?;
    }
    let [x, y, xs, ys] = coords;

    let (name, level) = match rec.w3.split_once(',') {
        Some((name, level)) => (name, scan_int(level).map(|(v, _)| v)),
        None => (rec.w3.as_str(), None),
    };
    if name.is_empty() {
        return Err(HeaderError("mob spawn without a name".to_string()));
    }
    let cut = cut_name(name, MOB_NAME_MAX);

    // A trailing `\t//comment` is still in w4; the scan stops at the tab.
    let w4 = rec.w4.split('\t').next().unwrap_or("");
    let mut w4 = w4.split(',');
    let mob = w4.next().map(str::trim).unwrap_or("");
    let amount = w4.next().and_then(scan_int).map(|(v, _)| v);
    let amount = match amount {
        Some(amount) if !mob.is_empty() => amount,
        _ => {
            return Err(HeaderError(format!(
                "mob spawn '{name}': needs 'id,amount', got '{}'",
                rec.w4
            )))
        }
    };
    let mut delays = [None; 2];
    let mut event = None;
    let mut extras = [None; 2];
    // Delays and extras are numbers; the event between them is any non-empty text.
    let mut clean = true;
    for slot in &mut delays {
        match w4.next().filter(|_| clean).and_then(scan_int) {
            Some((v, rest_empty)) => {
                *slot = Some(v);
                clean = rest_empty;
            }
            None => clean = false,
        }
    }
    if clean {
        event = w4.next().filter(|e| !e.is_empty()).map(str::to_string);
        if event.is_some() {
            for (slot, value) in extras.iter_mut().zip(scan_ints(w4)) {
                *slot = Some(value);
            }
        }
    }
    let [delay1, delay2] = delays;
    let [size, ai] = extras;
    Ok(MobHeader {
        map: map.to_string(),
        x,
        y,
        span: (xs, ys),
        boss,
        name: cut.text,
        name_truncated: cut.truncated,
        level,
        mob: mob.to_string(),
        amount,
        delay1,
        delay2,
        event,
        size,
        ai,
    })
}

/// `%d` conversions over comma-separated fields: yields values until a field doesn't start
/// with a number, stopping after a field with text left over (the next `,` wouldn't match).
fn scan_ints<'a>(fields: impl Iterator<Item = &'a str>) -> Vec<i64> {
    let mut clean = true;
    fields
        .map_while(|f| {
            if !clean {
                return None;
            }
            let (v, rest_empty) = scan_int(f)?;
            clean = rest_empty;
            Some(v)
        })
        .collect()
}

/// One `%d`: optional leading whitespace and sign, then digits. Returns the value and whether
/// nothing follows it in the field.
fn scan_int(field: &str) -> Option<(i64, bool)> {
    let s = field.trim_start();
    let digits_start = usize::from(s.starts_with(['-', '+']));
    let len = s[digits_start..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    if len == 0 {
        return None;
    }
    let end = digits_start + len;
    Some((s[..end].parse().ok()?, s.len() == end))
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

/// An NPC name split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub display: String,
    /// `None` when rAthena would generate one (empty, or still containing `::`).
    pub unique: Option<String>,
    /// A part was longer than `NPC_NAME_LENGTH` and was cut (rAthena warns).
    pub truncated: bool,
}

/// Splits `w3` at the first `::`; without it both parts are the full string. Each is cut to
/// `NPC_NAME_LENGTH` bytes (on a char boundary).
pub fn parse_name(w3: &str) -> ParsedName {
    let (display, unique) = match w3.split_once("::") {
        Some((d, u)) => (d, u),
        None => (w3, w3),
    };
    let display = cut_name(display, NPC_NAME_LENGTH);
    let unique = cut_name(unique, NPC_NAME_LENGTH);
    ParsedName {
        display: display.text,
        truncated: display.truncated || unique.truncated,
        unique: (!unique.text.is_empty() && !unique.text.contains("::")).then_some(unique.text),
    }
}

/// A name cut to a byte limit.
pub struct CutName {
    pub text: String,
    pub truncated: bool,
}

/// Cuts `s` to at most `max` bytes, on a char boundary.
pub fn cut_name(s: &str, max: usize) -> CutName {
    if s.len() <= max {
        return CutName {
            text: s.to_string(),
            truncated: false,
        };
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    CutName {
        text: s[..end].to_string(),
        truncated: true,
    }
}
