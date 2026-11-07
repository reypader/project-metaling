//! Look updates after gear changes, and the ammo icon.

use ro_core::{ServerEvent, UnitLooks};
use ro_db::ItemData;

use crate::ecs::Entity;
use crate::equipment::{EQP_HAND_L, EQP_HAND_R, EQP_SHOES};
use crate::map_instance::MapInstance;
use crate::protocol_consts::{
    LOOK_HEAD_BOTTOM, LOOK_HEAD_MID, LOOK_HEAD_TOP, LOOK_ROBE, LOOK_SHOES, LOOK_WEAPON,
};

/// The ammo icon (`EFST_CLIENT_ONLY_EQUIP_ARROW`), started on every ammo equip and map entry and
/// never ended, as in rAthena (spec §2.7).
const EFST_EQUIP_ARROW: u16 = 695;

const AMMO_ICON_STATE: u8 = 1;

const AMMO_ICON_MS: u32 = 9999;

pub(super) fn ammo_icon(gid: u32) -> ServerEvent {
    ServerEvent::StatusIconStarted {
        efst: EFST_EQUIP_ARROW,
        gid,
        state: AMMO_ICON_STATE,
        total_ms: AMMO_ICON_MS,
        remain_ms: AMMO_ICON_MS as i32,
        values: [0; 3],
    }
}

impl MapInstance {
    /// rAthena `pc_equiplookall` as `0x01d7` events: the hands, the worn shoes' item id, the
    /// head views and the robe, each with its current value.
    pub(crate) fn equip_look_events(&self, e: Entity) -> Vec<ServerEvent> {
        let gid = self.gid(e);
        let looks = self
            .world
            .players
            .get(e)
            .map(|info| info.looks)
            .unwrap_or_default();
        let shoes = self
            .world
            .inventories
            .get(e)
            .and_then(|inventory| inventory.worn_at(EQP_SHOES))
            .and_then(|slot| self.world.inventories.get(e)?.get(slot))
            .map_or(0, |item| {
                self.items
                    .get(item.item_id)
                    .map_or(item.item_id.0, ItemData::client_id)
            });
        [
            (LOOK_WEAPON, looks.weapon, looks.shield),
            (LOOK_SHOES, shoes, 0),
            (LOOK_HEAD_BOTTOM, looks.head_bottom.into(), 0),
            (LOOK_HEAD_TOP, looks.head_top.into(), 0),
            (LOOK_HEAD_MID, looks.head_mid.into(), 0),
            (LOOK_ROBE, looks.robe.into(), 0),
        ]
        .into_iter()
        .map(|(look_type, value, value2)| ServerEvent::SpriteChanged {
            gid,
            look_type,
            value,
            value2,
        })
        .collect()
    }

    /// Recomputes `e`'s looks from the worn-slot table and stores them. Returns (before, after).
    pub(crate) fn refresh_looks(&mut self, e: Entity) -> (UnitLooks, UnitLooks) {
        let new = self
            .world
            .inventories
            .get(e)
            .map(|inventory| inventory.gear(&self.items).unit_looks())
            .unwrap_or_default();
        let old = self
            .world
            .players
            .get_mut(e)
            .map(|info| std::mem::replace(&mut info.looks, new))
            .unwrap_or_default();
        (old, new)
    }

    /// One `LOOK_WEAPON` per hand in `mask` (a two-hander sends two identical ones), and
    /// `LOOK_SHOES` with `shoes` for the shoe position. Always sent, changed or not.
    pub(crate) fn send_hand_looks(&mut self, e: Entity, mask: u32, looks: &UnitLooks, shoes: u32) {
        for hand in [EQP_HAND_R, EQP_HAND_L] {
            if mask & hand != 0 {
                self.send_look(e, LOOK_WEAPON, looks.weapon, looks.shield);
            }
        }
        if mask & EQP_SHOES != 0 {
            self.send_look(e, LOOK_SHOES, shoes, 0);
        }
    }

    /// Head and robe looks, only for the views that changed (after the costume override).
    pub(crate) fn send_head_looks(&mut self, e: Entity, old: &UnitLooks, new: &UnitLooks) {
        for (look_type, before, after) in [
            (LOOK_HEAD_BOTTOM, old.head_bottom, new.head_bottom),
            (LOOK_HEAD_MID, old.head_mid, new.head_mid),
            (LOOK_HEAD_TOP, old.head_top, new.head_top),
            (LOOK_ROBE, old.robe, new.robe),
        ] {
            if before != after {
                self.send_look(e, look_type, after.into(), 0);
            }
        }
    }

    /// `0x01d7` to the area including self.
    pub(crate) fn send_look(&mut self, e: Entity, look_type: u8, value: u32, value2: u32) {
        let gid = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::SpriteChanged {
                gid,
                look_type,
                value,
                value2,
            },
        );
    }
}
