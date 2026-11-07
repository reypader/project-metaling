//! Per-map permanent mob spawn lines (`MOB_SPAWN_DB` YAML), produced offline by `tools/npc-import`
//! from rAthena's `monster` / `boss_monster` lines.
//!
//! The YAML keeps the line's own values. Rules that depend on map cells or config (spawn rectangle,
//! map-wide forcing, respawn delays) are applied at runtime.

use serde::{Deserialize, Serialize};

use crate::yaml::{is_false, is_zero};
use crate::MobId;

/// Largest `Amount` a spawn line may have.
const MAX_AMOUNT: u16 = 1000;
/// rAthena `MAX_LEVEL`.
const MAX_LEVEL: u16 = 275;
/// rAthena reads spans as `short`.
const MAX_SPAN: u16 = i16::MAX as u16;

/// One permanent spawn line: `amount` mobs of `mob_id` around `(x, y)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMobSpawn", into = "RawMobSpawn")]
pub struct MobSpawn {
    /// Name shown to clients (<= 23 chars; `--en--` / `--ja--` already resolved).
    pub name: String,
    pub mob_id: MobId,
    /// Center. `(0, 0)` means map-wide.
    pub x: u16,
    pub y: u16,
    /// Raw `(xs, ys)` from the line, before the runtime rectangle rules. Each `<= 32767`.
    pub span: (u16, u16),
    /// `1..=1000`.
    pub amount: u16,
    /// Base respawn delay in ms.
    pub delay1: u32,
    /// Respawn variance in ms.
    pub delay2: u32,
    /// Level from the line (`1..=275`); `None` means the mob_db level.
    pub level: Option<u16>,
    /// The line used `boss_monster` (not mob_db's `Class: Boss`).
    pub boss: bool,
    /// NPC label run when the mob dies (`npc::OnLabel`).
    pub event: Option<String>,
    /// `<rAthena path>:<line>` of the spawn line.
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct RawSpan {
    x: u16,
    y: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct RawMobSpawn {
    name: String,
    mob_id: u32,
    x: u16,
    y: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<RawSpan>,
    amount: u16,
    delay1: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    delay2: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    level: Option<u16>,
    #[serde(default, skip_serializing_if = "is_false")]
    boss: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event: Option<String>,
    source: String,
}

impl TryFrom<RawMobSpawn> for MobSpawn {
    type Error = String;

    fn try_from(raw: RawMobSpawn) -> Result<Self, Self::Error> {
        let mob_id = MobId(raw.mob_id);
        if !mob_id.is_valid() {
            return Err(format!(
                "spawn '{}': MobId {} is outside 1001..=3998 and 20021..=31998",
                raw.name, raw.mob_id
            ));
        }
        if !(1..=MAX_AMOUNT).contains(&raw.amount) {
            return Err(format!(
                "spawn '{}': Amount {} is outside 1..={MAX_AMOUNT}",
                raw.name, raw.amount
            ));
        }
        if let Some(span) = raw.span.filter(|s| s.x > MAX_SPAN || s.y > MAX_SPAN) {
            return Err(format!(
                "spawn '{}': Span ({}, {}) above {MAX_SPAN}",
                raw.name, span.x, span.y
            ));
        }
        if let Some(level) = raw.level.filter(|l| !(1..=MAX_LEVEL).contains(l)) {
            return Err(format!(
                "spawn '{}': Level {level} is outside 1..={MAX_LEVEL}",
                raw.name
            ));
        }
        Ok(Self {
            name: raw.name,
            mob_id,
            x: raw.x,
            y: raw.y,
            span: raw.span.map_or((0, 0), |s| (s.x, s.y)),
            amount: raw.amount,
            delay1: raw.delay1,
            delay2: raw.delay2,
            level: raw.level,
            boss: raw.boss,
            event: raw.event,
            source: raw.source,
        })
    }
}

impl From<MobSpawn> for RawMobSpawn {
    fn from(spawn: MobSpawn) -> Self {
        let (xs, ys) = spawn.span;
        Self {
            name: spawn.name,
            mob_id: spawn.mob_id.0,
            x: spawn.x,
            y: spawn.y,
            span: (xs != 0 || ys != 0).then_some(RawSpan { x: xs, y: ys }),
            amount: spawn.amount,
            delay1: spawn.delay1,
            delay2: spawn.delay2,
            level: spawn.level,
            boss: spawn.boss,
            event: spawn.event,
            source: spawn.source,
        }
    }
}
