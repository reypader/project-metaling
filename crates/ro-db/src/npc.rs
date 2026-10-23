//! Per-map NPC spawn lists (`NPC_SPAWN_DB` YAML), produced offline by `tools/npc-import` from
//! rAthena's NPC files. Only headers: where an NPC stands, how it looks, and where a warp leads.
//!
//! The YAML is flat (`Kind` plus an optional `Warp` block); `NpcSpawn` is the checked form.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::yaml::{is_false, Schema};
use crate::DbLoadError;

/// `Header.Type` of an NPC spawn file.
pub const NPC_SPAWN_DB_TYPE: &str = "NPC_SPAWN_DB";
/// `Header.Version` this crate reads and writes.
pub const NPC_SPAWN_DB_VERSION: u32 = 1;

/// rAthena header state suffix: `script(HIDDEN)` and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NpcState {
    #[default]
    Normal,
    /// Sent with `effect_state` `OPTION_HIDE`.
    Hidden,
    /// Sent with `effect_state` `OPTION_CLOAK`.
    Cloaked,
    /// Never sent; does not trigger.
    Disabled,
}

/// Where a warp leads. `map` is a bare rAthena map name (`prt_fild08`, no `.gat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpDest {
    pub map: String,
    pub x: u16,
    pub y: u16,
    /// rAthena `warp2`: also triggers for hiding/cloaked players.
    pub trigger_while_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcKind {
    Script,
    Shop,
    Warp(WarpDest),
}

/// One positioned NPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawNpcSpawn", into = "RawNpcSpawn")]
pub struct NpcSpawn {
    /// Display name as rAthena's `nd->name` (includes any `#suffix`; the client hides it).
    pub name: String,
    /// rAthena `exname`; equal to `name` unless the header used `name::unique`.
    pub unique_name: String,
    pub x: u16,
    pub y: u16,
    /// Raw facing from the file, not validated (data has values above 7).
    pub dir: u8,
    /// Class id sent as `job`. `-1` and `32767` are never sent.
    pub sprite: i16,
    /// The sprite is a mob id (`object_type` `0x0C` instead of `0x06`).
    pub sprite_is_mob: bool,
    pub state: NpcState,
    /// OnTouch/warp half-extents `(xs, ys)`: the area is `(2xs+1) x (2ys+1)`.
    pub trigger: Option<(u16, u16)>,
    /// Unique name of the `duplicate(...)` source.
    pub duplicate_of: Option<String>,
    pub kind: NpcKind,
    /// `<rAthena path>:<line>` of the header.
    pub source: String,
}

/// All NPCs of one map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapNpcs {
    pub npcs: Vec<NpcSpawn>,
}

const SCHEMA: Schema = Schema {
    db_type: NPC_SPAWN_DB_TYPE,
    version: NPC_SPAWN_DB_VERSION,
};

impl MapNpcs {
    /// Parses an `NPC_SPAWN_DB` file. `path` is only a label for errors.
    pub fn from_yaml_str(path: &str, text: &str) -> Result<Self, DbLoadError> {
        SCHEMA.parse(path, text).map(|npcs| Self { npcs })
    }

    /// Reads and parses one map's file. A missing file is `Ok(None)` (the map has no NPCs).
    pub fn load_yaml(path: &Path) -> Result<Option<Self>, DbLoadError> {
        Ok(SCHEMA.load(path)?.map(|npcs| Self { npcs }))
    }

    /// Serializes with the current header (no leading comments; the importer adds those).
    pub fn to_yaml_string(&self) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(&self.npcs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum RawKind {
    Script,
    Shop,
    Warp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct RawSpan {
    x: u16,
    y: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct RawWarp {
    map: String,
    x: u16,
    y: u16,
    trigger_while_hidden: bool,
}

fn is_normal(s: &NpcState) -> bool {
    *s == NpcState::Normal
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct RawNpcSpawn {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unique_name: Option<String>,
    x: u16,
    y: u16,
    dir: u8,
    sprite: i16,
    #[serde(default, skip_serializing_if = "is_false")]
    sprite_is_mob: bool,
    #[serde(default, skip_serializing_if = "is_normal")]
    state: NpcState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trigger: Option<RawSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duplicate_of: Option<String>,
    kind: RawKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    warp: Option<RawWarp>,
    source: String,
}

impl TryFrom<RawNpcSpawn> for NpcSpawn {
    type Error = String;

    fn try_from(raw: RawNpcSpawn) -> Result<Self, Self::Error> {
        let kind = match (raw.kind, raw.warp) {
            (RawKind::Warp, Some(w)) => NpcKind::Warp(WarpDest {
                map: w.map,
                x: w.x,
                y: w.y,
                trigger_while_hidden: w.trigger_while_hidden,
            }),
            (RawKind::Warp, None) => {
                return Err(format!("NPC '{}': Kind Warp needs a Warp block", raw.name))
            }
            (_, Some(_)) => {
                return Err(format!(
                    "NPC '{}': only Kind Warp may have a Warp block",
                    raw.name
                ))
            }
            (RawKind::Script, None) => NpcKind::Script,
            (RawKind::Shop, None) => NpcKind::Shop,
        };
        Ok(Self {
            unique_name: raw.unique_name.unwrap_or_else(|| raw.name.clone()),
            name: raw.name,
            x: raw.x,
            y: raw.y,
            dir: raw.dir,
            sprite: raw.sprite,
            sprite_is_mob: raw.sprite_is_mob,
            state: raw.state,
            trigger: raw.trigger.map(|s| (s.x, s.y)),
            duplicate_of: raw.duplicate_of,
            kind,
            source: raw.source,
        })
    }
}

impl From<NpcSpawn> for RawNpcSpawn {
    fn from(npc: NpcSpawn) -> Self {
        let (kind, warp) = match npc.kind {
            NpcKind::Script => (RawKind::Script, None),
            NpcKind::Shop => (RawKind::Shop, None),
            NpcKind::Warp(w) => (
                RawKind::Warp,
                Some(RawWarp {
                    map: w.map,
                    x: w.x,
                    y: w.y,
                    trigger_while_hidden: w.trigger_while_hidden,
                }),
            ),
        };
        Self {
            unique_name: (npc.unique_name != npc.name).then_some(npc.unique_name),
            name: npc.name,
            x: npc.x,
            y: npc.y,
            dir: npc.dir,
            sprite: npc.sprite,
            sprite_is_mob: npc.sprite_is_mob,
            state: npc.state,
            trigger: npc.trigger.map(|(x, y)| RawSpan { x, y }),
            duplicate_of: npc.duplicate_of,
            kind,
            warp,
            source: npc.source,
        }
    }
}
