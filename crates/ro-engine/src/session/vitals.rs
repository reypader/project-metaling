//! HP, SP and zeny changes outside the stat pass: heals and cast costs (spec
//! `specs/effects/spec.md` §2.4, §2.8, §2.9; `@heal`/`@zeny` are in `vitals_commands.rs`). These
//! are vitals: nothing here marks the player dirty, and the play-state save picks the values up
//! (§2.10).

use ro_core::{ServerEvent, MAX_ZENY};

use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::protocol_consts::{SP_HP, SP_SP, SP_ZENY};
use crate::stats::param;

impl MapInstance {
    /// Restores up to `hp` and `sp` on player `e`, clamped to the max. Pushes `SP_HP` then
    /// `SP_SP` to `e`, each only when it changed.
    pub(crate) fn heal_player(&mut self, e: Entity, hp: u32, sp: u32) {
        self.change_vitals(e, i64::from(hp), i64::from(sp), 0);
    }

    /// Adds `hp` and `sp` (either may be negative) to player `e`: HP stays in `[hp_floor, max]`
    /// and SP in `[0, max]`. Pushes `SP_HP` then `SP_SP` to `e`, each only when it changed.
    pub(crate) fn change_vitals(&mut self, e: Entity, hp: i64, sp: i64, hp_floor: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let status = &mut info.status;
        let clamp = |value: u32, delta: i64, floor: u32, max: u32| -> u32 {
            let next =
                (i64::from(value) + delta).clamp(i64::from(floor), i64::from(max.max(floor)));
            u32::try_from(next).unwrap_or(value)
        };
        let (old_hp, old_sp) = (status.hp, status.sp);
        status.hp = clamp(status.hp, hp, hp_floor, status.max_hp);
        status.sp = clamp(status.sp, sp, 0, status.max_sp);
        let (new_hp, new_sp) = (status.hp, status.sp);
        if new_hp != old_hp {
            self.emit_self(e, param(SP_HP, new_hp));
        }
        if new_sp != old_sp {
            self.emit_self(e, param(SP_SP, new_sp));
        }
    }

    /// Takes `amount` SP off player `e` (at most what it has) and pushes `SP_SP`.
    pub(crate) fn spend_sp(&mut self, e: Entity, amount: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.sp = info.status.sp.saturating_sub(amount);
        let sp = info.status.sp;
        self.emit_self(e, param(SP_SP, sp));
    }

    /// Takes `amount` HP off player `e`, leaving at least 1, and pushes `SP_HP`.
    pub(crate) fn spend_hp(&mut self, e: Entity, amount: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.hp = info.status.hp.saturating_sub(amount).max(1);
        let hp = info.status.hp;
        self.emit_self(e, param(SP_HP, hp));
    }

    /// Player `e`'s zeny; 0 for a non-player.
    pub(crate) fn zeny(&self, e: Entity) -> u32 {
        self.world.players.get(e).map_or(0, |info| info.status.zeny)
    }

    /// Sets player `e`'s zeny (capped at `MAX_ZENY`) and pushes `0x00b1` with the new total.
    pub(crate) fn set_zeny(&mut self, e: Entity, zeny: u32) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.status.zeny = zeny.min(MAX_ZENY);
        let value = i32::try_from(info.status.zeny).unwrap_or(i32::MAX);
        self.emit_self(
            e,
            ServerEvent::LongParameterChanged {
                var_id: SP_ZENY,
                value,
            },
        );
    }
}
