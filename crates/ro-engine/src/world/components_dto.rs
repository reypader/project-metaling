//! Component data stored per entity kind in the `World`: identity, appearance and per-kind state.

use std::sync::Arc;

use ro_core::{AccountId, CharId, CharacterStatus, InventoryItem, Sex, UnitLooks, UnitObjectType};
use ro_db::{MobId, NpcState};

use crate::combat::DamageLog;
use crate::equipment::GearStatus;
use crate::mob::MobStats;
use crate::skill::SkillSet;
use crate::stats::FollowUp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityKind {
    /// Unused slot.
    Free,
    Player,
    Npc,
    Mob,
    FloorItem,
}

/// Static appearance and identity of a player entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerInfo {
    pub(crate) account_id: AccountId,
    pub(crate) char_id: CharId,
    pub(crate) name: Arc<str>,
    pub(crate) sex: Sex,
    pub(crate) job: u16,
    pub(crate) hair_style: u16,
    pub(crate) hair_color: u16,
    pub(crate) clevel: u16,
    /// Allocated stats, job level, points, exp and HP/SP (the persisted progress).
    pub(crate) status: CharacterStatus,
    /// Account permission group; 99 may use every GM command.
    pub(crate) group_id: i32,
    /// The status snapshot committed last (spec `specs/stats/spec.md` §2.1): computed at spawn,
    /// then by the tick's recalc pass, which diffs against it.
    pub(crate) derived: ro_mechanics::Status,
    /// The allocated STR..LUK then POW..CRT `derived` was computed from: the diff sends a
    /// stat's `0x0141` when its allocated base or its battle value changed.
    pub(crate) allocated: [u16; 12],
    /// The worn gear's part of the status, rebuilt only when an equip change marks it stale.
    pub(crate) gear: GearStatus,
    /// `stats::DIRTY` / `STALE_GEAR` / `SKILLS` / `LISTED`: what the tick's recalc pass owes
    /// this player.
    pub(crate) stat_flags: u8,
    /// Packets and actions a flow queued for after this tick's recalc (spec §2.1 "Follow-up
    /// queue"). Drained in Stage 4; the capacity is reused across ticks.
    pub(crate) followups: Vec<FollowUp>,
    /// What other players see, as last sent (spec §2.1). Built at spawn and refreshed by every
    /// equip change; entry packets read it, never the inventory.
    pub(crate) looks: UnitLooks,
    /// Consecutive warps that landed inside a warp area (rAthena `count_rewarp`).
    pub(crate) rewarp_count: u8,
    /// First entry after a login (rAthena `connect_new`): the next actor init sends the skill
    /// list and hotkeys.
    pub(crate) connect_new: bool,
    /// Arrived from login or another map (rAthena `changemap`): the next actor init sends the
    /// config flags and reputation list. Not set by a same-map warp.
    pub(crate) changemap: bool,
    /// Learned and visible skills (spec `specs/skills/spec.md` §2.1).
    pub(crate) skills: SkillSet,
    /// The job level at each class change (2nd, 3rd, 4th); 0 = unset.
    pub(crate) change_level: [u16; 3],
    /// rAthena `canuseitem_tick`: no item use before this time (ms), shared by every item.
    pub(crate) canuseitem_at: u64,
}

/// Static appearance and identity of an NPC entity (warps included).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcInfo {
    pub(crate) gid: u32,
    pub(crate) name: Arc<str>,
    pub(crate) object_type: UnitObjectType,
    /// Class id sent as `job`.
    pub(crate) sprite: u16,
    /// `effect_state` (`sc.option`): hidden/cloaked NPCs.
    pub(crate) effect_state: i32,
    /// Initial state. Warp map change must skip `Disabled` and `Cloaked` warps
    /// (rAthena `npc_touch_areanpc`).
    pub(crate) state: NpcState,
}

/// Identity and state of a mob entity, with HP and level from mob_db.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MobInfo {
    pub(crate) gid: u32,
    pub(crate) mob_id: MobId,
    /// Shared with the spawn group.
    pub(crate) name: Arc<str>,
    /// The spawn-line level, else mob_db `Level`.
    pub(crate) level: u16,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    /// rAthena `bosstype` for the entry packets.
    pub(crate) is_boss: u8,
    /// Index into `MapInstance::spawn_groups`: the line this mob respawns from.
    pub(crate) spawn_group: u32,
    /// Battle status and defender view, shared per mob id.
    pub(crate) stats: Arc<MobStats>,
    /// Who hit it and for how much (spec §2.7).
    pub(crate) damage_log: DamageLog,
    /// Killed: out of the grid, waiting for its death vanish (spec §2.8).
    pub(crate) dying: bool,
}

/// An item lying on the floor (rAthena `flooritem_data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FloorItem {
    /// Object id, `[2, 2_000_000)`; also the entity's gid.
    pub(crate) id: u32,
    /// The item; `amount` is the floor amount.
    pub(crate) item: InventoryItem,
    /// Sub-cell offsets, `rnd[1,4] × 3`.
    pub(crate) sub_x: u8,
    pub(crate) sub_y: u8,
    /// Loot owners by tier (char ids); `None` = nobody.
    pub(crate) owners: [Option<CharId>; 3],
    /// When each tier's wait ends (ms).
    pub(crate) owner_ticks: [u64; 3],
    /// Removal time (ms).
    pub(crate) expire_at: u64,
    /// The mob that dropped it; 0 for player drops.
    pub(crate) mob_id: u32,
}

/// A warp's touch area and destination, resolved at spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WarpTrigger {
    /// `(xs, ys)`: the area is `(2xs+1) x (2ys+1)` around the warp's cell.
    pub(crate) half_extents: (u16, u16),
    /// Destination map as a `.gat` name.
    pub(crate) dest_map: Arc<str>,
    /// Raw destination cell; `(0, 0)` or out of bounds means a random cell.
    pub(crate) dest: (u16, u16),
    /// `warp2`: fires for hidden players too (no hiding state yet).
    pub(crate) trigger_while_hidden: bool,
    /// `dest_map` is this instance's map.
    pub(crate) same_map: bool,
}
