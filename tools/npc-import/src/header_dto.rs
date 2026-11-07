//! What parsing a script header line yields.

use ro_db::NpcState;

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

/// An NPC name split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub display: String,
    /// `None` when rAthena would generate one (empty, or still containing `::`).
    pub unique: Option<String>,
    /// A part was longer than `NPC_NAME_LENGTH` and was cut (rAthena warns).
    pub truncated: bool,
}

/// A name cut to a byte limit.
pub struct CutName {
    pub text: String,
    pub truncated: bool,
}
