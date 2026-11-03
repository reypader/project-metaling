//! Using an item from the inventory (`0x0439`; spec `specs/effects/spec.md` §2.7, catalog
//! `mechanics/inventory.md` §7.1-§7.4).
//!
//! The gates come first; a refused use gets the failure `0x01c8` and nothing else. The item's
//! definition is resolved through `ro-scripting` before anything is sent, so a hook error is a
//! refusal too. Then the ack goes to the area, one item is deleted (`SP_WEIGHT`, no `0x07fa`)
//! and the effects apply. A use stops neither a walk nor the attack loop, and works while
//! sitting.

use std::mem;
use std::sync::Arc;

use ro_core::{ItemId, ServerEvent};
use ro_db::{ItemData, ItemType, ItemUse as DataUse};
use ro_scripting::{CastSource, HookCtx, ItemUse};
use tracing::warn;

use crate::cast::MapRoll;
use crate::engine::{MapInstance, SP_WEIGHT};
use crate::entity::Entity;
use crate::equipment::{usable_by, Wearer};
use crate::inventory::slot_of;
use crate::job::job_mapid;

/// A use that passed the gates: the definition kind and its hook arguments.
struct Use {
    kind: ItemUse,
    args: [i32; 4],
}

impl MapInstance {
    /// `CZ_USE_ITEM` (`0x0439`, spec §2.7).
    pub(crate) fn use_item(&mut self, e: Entity, index: u16, now: u64) {
        // rAthena answers a dead player with a dead-state refresh and no ack; player death
        // doesn't exist yet.
        if self.is_dead(e) {
            return;
        }
        // An index out of range, an empty slot or a stack of 0 is silent (the ack builder sends
        // nothing).
        let Some(slot) = slot_of(index) else {
            return;
        };
        let Some(stack) = self.world.inventories.get(e).and_then(|inv| inv.get(slot)) else {
            return;
        };
        if stack.amount == 0 {
            return;
        }
        let (item_id, amount, rental) = (stack.item_id, stack.amount, stack.expire_time != 0);
        let items = Arc::clone(&self.items);
        let data = items.get(item_id);
        let ack = |this: &Self, amount: u16, ok: bool| ServerEvent::ItemUseAck {
            index,
            item_id: data.map_or(item_id.0, ItemData::client_id),
            gid: this.gid(e),
            amount: i16::try_from(amount).unwrap_or(i16::MAX),
            ok,
        };
        let refuse = |this: &mut Self| {
            let event = ack(this, 0, false);
            this.emit_self(e, event);
        };
        let Some(chosen) = data.and_then(|data| self.usable(e, data, now)) else {
            return refuse(self);
        };

        // Resolve completely before anything is sent or spent (as at castend).
        let Some((definitions, hooks)) = self.scripts.get() else {
            return refuse(self);
        };
        let mut effects = mem::take(&mut self.effect_buf);
        effects.clear();
        let view = self.unit_view(e);
        let Some(info) = self.world.players.get(e) else {
            self.effect_buf = effects;
            return;
        };
        let ctx = HookCtx {
            caster: view,
            target: view,
            skill: 0,
            level: 0,
            source: CastSource::ItemUse(item_id),
            args: chosen.args,
            skills: &info.skills,
        };
        let resolved = definitions.item_use(chosen.kind).resolve(
            &ctx,
            &*hooks,
            &mut MapRoll(&mut self.rng),
            &mut effects,
        );
        if let Err(err) = resolved {
            warn!(map = %self.map_name, item = item_id.0, %err, "item use hook failed");
            self.effect_buf = effects;
            return refuse(self);
        }

        if let Some(info) = self.world.players.get_mut(e) {
            info.canuseitem_at = now + u64::from(self.battle.item_use_interval);
        }
        if rental {
            // rAthena keeps a rental and answers with the failure form; its script still runs.
            refuse(self);
        } else {
            let event = ack(self, amount.saturating_sub(1), true);
            self.emit_aoi(e, false, event);
            if !self.consume_one(e, slot) {
                self.effect_buf = effects;
                return;
            }
        }
        for &effect in effects.iter() {
            self.apply_effect(e, e, 0, 0, effect, now);
        }
        self.effect_buf = effects;
    }

    /// The `pc_isUseitem` gates that apply (catalog §7.2): a usable type, the use interval, the
    /// user restrictions, and a lifted `Use` (a deviation: rAthena runs any script).
    fn usable(&self, e: Entity, data: &ItemData, now: u64) -> Option<Use> {
        if !matches!(
            data.item_type,
            ItemType::Healing | ItemType::Usable | ItemType::Cash
        ) {
            return None;
        }
        let info = self.world.players.get(e)?;
        if now < info.canuseitem_at {
            return None;
        }
        let user = Wearer {
            base_level: info.clevel,
            sex: info.sex,
            mapid: job_mapid(info.job),
        };
        if !usable_by(data, &user) {
            return None;
        }
        let chosen = match data.on_use? {
            DataUse::ItemHeal { hp, sp } => {
                let arg = |v: u32| i32::try_from(v).ok();
                let (Some(a), Some(b), Some(c), Some(d)) =
                    (arg(hp.0), arg(hp.1), arg(sp.0), arg(sp.1))
                else {
                    return overflow(data.id);
                };
                Use {
                    kind: ItemUse::ItemHeal,
                    args: [a, b, c, d],
                }
            }
            DataUse::PercentHeal { hp, sp } => Use {
                kind: ItemUse::PercentHeal,
                args: [i32::from(hp), i32::from(sp), 0, 0],
            },
        };
        Some(chosen)
    }

    /// Deletes one item from `slot` the way a use does: `SP_WEIGHT`, no `0x07fa`.
    /// Removes one item from `slot` and pushes `SP_WEIGHT`; false (and a warning) when it
    /// can't, and then the use applies nothing.
    fn consume_one(&mut self, e: Entity, slot: usize) -> bool {
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            return false;
        };
        if inventory.remove(slot, 1, &self.items).is_none() {
            warn!(map = %self.map_name, slot, "item use: removal failed after the gates passed");
            return false;
        }
        let weight = inventory.weight();
        self.emit_weight(e, SP_WEIGHT, weight);
        true
    }
}

fn overflow(item: ItemId) -> Option<Use> {
    warn!(
        item = item.0,
        "item use: heal range does not fit a hook argument"
    );
    None
}
