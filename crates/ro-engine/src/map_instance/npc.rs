//! NPC spawning, name requests, entry events and cell changes.

use std::sync::Arc;

use ro_core::{gat_map_name, MapPosition, ServerEvent, UnitObjectType};
use ro_db::{CellType, MapNpcs, NpcKind, NpcSpawn, NpcState};
use tracing::warn;

use super::MapInstance;
use crate::ecs::{Entity, AREA_SIZE};
use crate::error::EngineError;
use crate::world::{NpcInfo, WarpTrigger};

/// rAthena `DEFAULT_NPC_WALK_SPEED`, sent as an NPC's speed.
const NPC_SPEED: u16 = 200;

/// NPC classes the client is never sent: `JT_FAKENPC` and `JT_INVISIBLE`.
const FAKE_NPC_SPRITE: i16 = -1;

const INVISIBLE_SPRITE: i16 = 32767;

/// `effect_state` bits (`sc.option`) for hidden and cloaked NPCs.
const OPTION_HIDE: i32 = 0x2;

const OPTION_CLOAK: i32 = 0x4;

impl MapInstance {
    /// How `subject` looks to an observer that just started seeing it.
    pub(crate) fn entry_event(&self, subject: Entity, tick: u32) -> Option<ServerEvent> {
        if self.is_floor_item(subject) {
            return self
                .floor_appearance(subject)
                .map(|item| ServerEvent::FloorItemEntered { item });
        }
        let unit = self.appearance(subject)?;
        Some(match self.walk_dest(subject) {
            Some(dest) => ServerEvent::UnitWalking {
                start: (unit.pos.x, unit.pos.y),
                unit,
                dest,
                move_start_tick: tick,
            },
            None => ServerEvent::UnitStanding { unit },
        })
    }

    pub(crate) fn name_request(&mut self, e: Entity, gid: u32) {
        let Some(target) = self.world.entity_by_gid(gid) else {
            return;
        };
        if !self.world.in_grid[target.index()] {
            return;
        }
        let (me, them) = (self.pos(e), self.pos(target));
        if me.x.abs_diff(them.x) > AREA_SIZE || me.y.abs_diff(them.y) > AREA_SIZE {
            return;
        }
        // Mobs answer like NPCs; mob_db `GroupId`/`Title` are not loaded yet.
        let npc_like = self
            .world
            .npcs
            .get(target)
            .map(|n| &n.name)
            .or_else(|| self.world.mobs.get(target).map(|m| &m.name));
        if let Some(name) = npc_like {
            let name = Arc::clone(name);
            self.emit_self(e, ServerEvent::AckNpcName { gid, name });
            return;
        }
        let Some(name) = self.world.players.get(target).map(|p| p.name.to_string()) else {
            return;
        };
        self.emit_self(
            e,
            ServerEvent::AckEntityName {
                gid,
                name,
                party_name: String::new(),
                guild_name: String::new(),
                position_name: String::new(),
                title_id: 0,
            },
        );
    }

    /// Creates the map's static NPCs (spec ENG-NPC-01). Emits nothing: no player is on the map
    /// yet, and each one sees the NPCs when it enters or walks into range. Entries outside the
    /// grid are skipped with a warning. Returns how many were created.
    pub fn spawn_npcs(&mut self, npcs: &MapNpcs) -> usize {
        let mut created = 0;
        for npc in &npcs.npcs {
            match self.spawn_npc(npc) {
                Ok(()) => created += 1,
                Err(err) => warn!(
                    map = %self.map_name,
                    npc = %npc.name,
                    source = %npc.source,
                    "NPC not spawned: {err}"
                ),
            }
        }
        created
    }

    fn spawn_npc(&mut self, npc: &NpcSpawn) -> Result<(), EngineError> {
        if npc.x >= self.walkability.width() || npc.y >= self.walkability.height() {
            return Err(EngineError::InvalidCell { x: npc.x, y: npc.y });
        }
        let gid = self.next_npc_id;
        let object_type = if npc.sprite_is_mob {
            UnitObjectType::MobSpriteNpc
        } else {
            UnitObjectType::Npc
        };
        let effect_state = match npc.state {
            NpcState::Hidden => OPTION_HIDE,
            NpcState::Cloaked => OPTION_CLOAK,
            NpcState::Normal | NpcState::Disabled => 0,
        };
        // A warp without a (non-negative) span has no touch area, like rAthena's `npc_setcells`.
        let warp = match (&npc.kind, npc.trigger) {
            (NpcKind::Warp(dest), Some(half_extents)) => {
                let dest_map = gat_map_name(&dest.map);
                Some(WarpTrigger {
                    half_extents,
                    same_map: *dest_map == *self.gat_name,
                    dest_map: Arc::from(dest_map),
                    dest: (dest.x, dest.y),
                    trigger_while_hidden: dest.trigger_while_hidden,
                })
            }
            _ => None,
        };
        let info = NpcInfo {
            gid,
            name: Arc::from(npc.name.as_str()),
            object_type,
            // -1 is never sent, so its wire value doesn't matter.
            sprite: u16::try_from(npc.sprite).unwrap_or(0),
            effect_state,
            state: npc.state,
        };
        let pos = MapPosition {
            x: npc.x,
            y: npc.y,
            dir: npc.dir,
        };
        let area = warp.as_ref().map(|w| w.half_extents);
        let e = self.world.spawn_npc(info, pos, NPC_SPEED, warp)?;
        if let Some(half_extents) = area {
            self.mark_warp_cells((npc.x, npc.y), half_extents);
        }
        self.next_npc_id = self.next_npc_id.wrapping_add(1);
        let never_sent = npc.sprite == FAKE_NPC_SPRITE
            || npc.sprite == INVISIBLE_SPRITE
            || npc.state == NpcState::Disabled;
        if !never_sent {
            self.world.show(e);
        }
        Ok(())
    }

    /// Changes one cell at runtime (rAthena `setcell`; later Ice Wall and similar). Copies the
    /// grid first if other instances share it. Paths already planned are not re-checked until
    /// their next repath. Returns `false` when out of bounds.
    pub fn set_cell(&mut self, x: u16, y: u16, cell: CellType) -> bool {
        Arc::make_mut(&mut self.walkability).set(x, y, cell)
    }
}

#[cfg(test)]
mod tests;
