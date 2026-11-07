//! How a unit looks to other clients: the shared parts of the unit entry packets.

use std::sync::Arc;

use crate::ids_dto::{CharId, MapPosition, Sex};

/// What a unit is to the client: the entry packets' `object_type` (rAthena `clif_bl_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitObjectType {
    Pc,
    Npc,
    /// An NPC whose sprite is a mob id.
    MobSpriteNpc,
    Mob,
}

/// What other players see a unit wearing: the gear fields of the entry packets. Weapon and
/// shield are client item ids (`ViewId` or item id), not types; see
/// [`CharacterLooks`](crate::CharacterLooks) for the char-select form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitLooks {
    /// The right-hand item; 0 for none.
    pub weapon: u32,
    /// The left-hand item; 0 for none or when a two-hander fills both hands.
    pub shield: u32,
    /// View ids after the costume override (`accessory`, `accessory2`, `accessory3`).
    pub head_bottom: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub robe: u16,
}

/// Appearance snapshot shared by the unit entry packets (`0x09fd`/`0x09fe`/`0x09ff`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAppearance {
    pub object_type: UnitObjectType,
    /// Wire id (`bl.id`): the account_id for players, the NPC/mob id for NPCs and mobs.
    pub gid: u32,
    /// Written to the `GID` field; `None` (non-players) writes 0.
    pub char_id: Option<CharId>,
    /// Shared, so entry events don't copy the string.
    pub name: Arc<str>,
    pub job: u16,
    pub sex: Sex,
    pub hair_style: u16,
    pub hair_color: u16,
    /// 0 straight, 1 clockwise, 2 counter-clockwise.
    pub head_dir: u16,
    /// Cell and body direction.
    pub pos: MapPosition,
    pub speed: u16,
    /// rAthena `dead_sit`: 0 standing, 1 dead, 2 sitting. Goes only to the `0x09ff` `state`
    /// byte (offset 68). It is NOT the wire `body_state` (`sc.opt1`, offset 15), which stays 0.
    pub dead_sit: u8,
    pub clevel: u16,
    /// rAthena `bosstype`: 1 for a boss mob, 2 for an MVP, 0 otherwise.
    pub is_boss: u8,
    /// rAthena `sc.option` (`effect_state`): `0x2` hidden, `0x4` cloaked NPCs.
    pub effect_state: i32,
    /// Worn gear; all 0 for NPCs and mobs.
    pub looks: UnitLooks,
}
