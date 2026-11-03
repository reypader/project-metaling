//! HP, SP and zeny changes outside the stat pass: heals, cast costs, `@heal` and `@zeny` (spec
//! `specs/effects/spec.md` §2.4, §2.8, §2.9). These are vitals: nothing here marks the player
//! dirty, and the play-state save picks the values up (§2.10).

use ro_core::{ServerEvent, MAX_ZENY};

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::status::{param, SP_HP, SP_SP};

/// `SP_ZENY`, sent with `0x00b1`.
pub(crate) const SP_ZENY: u16 = 20;

// Our wording.
const MSG_ZENY_USAGE: &str = "Usage: @zeny <amount> (a negative amount removes zeny)";
const MSG_ZENY_CHANGED: &str = "Your zeny changed.";
const MSG_ZENY_OUT_OF_RANGE: &str = "That would take your zeny out of range.";
const MSG_HEAL_USAGE: &str = "Usage: @heal [<hp> [<sp>]]";
const MSG_HEALED: &str = "HP and SP restored.";
const MSG_HEAL_CHANGED: &str = "HP and SP changed.";

impl MapInstance {
    /// Restores up to `hp` and `sp` on player `e`, clamped to the max. Pushes `SP_HP` then
    /// `SP_SP` to `e`, each only when it changed.
    pub(crate) fn heal_player(&mut self, e: Entity, hp: u32, sp: u32) {
        self.change_vitals(e, i64::from(hp), i64::from(sp), 0);
    }

    /// Adds `hp` and `sp` (either may be negative) to player `e`: HP stays in `[hp_floor, max]`
    /// and SP in `[0, max]`. Pushes `SP_HP` then `SP_SP` to `e`, each only when it changed.
    fn change_vitals(&mut self, e: Entity, hp: i64, sp: i64, hp_floor: u32) {
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

    /// `@zeny <n>` (spec §2.9): adds `n` (negative removes). A result outside `[0, MAX_ZENY]`
    /// is refused with a message only.
    pub(crate) fn command_zeny(&mut self, e: Entity, params: &str) {
        let amount = params
            .split_whitespace()
            .next()
            .and_then(|token| token.parse::<i64>().ok())
            .filter(|&n| n != 0);
        let Some(amount) = amount else {
            return self.tell(e, MSG_ZENY_USAGE);
        };
        let next = i64::from(self.zeny(e)).saturating_add(amount);
        let Some(next) = u32::try_from(next).ok().filter(|&z| z <= MAX_ZENY) else {
            return self.tell(e, MSG_ZENY_OUT_OF_RANGE);
        };
        self.set_zeny(e, next);
        self.tell(e, MSG_ZENY_CHANGED);
    }

    /// `@heal [hp [sp]]` (spec §2.9): no argument fills HP and SP; otherwise each is added
    /// (negative removes), HP kept at 1 or more (there is no player death yet).
    pub(crate) fn command_heal(&mut self, e: Entity, params: &str) {
        let mut tokens = params.split_whitespace();
        let mut next = || tokens.next().map(str::parse::<i64>);
        let (hp, sp) = match (next(), next()) {
            (None, _) => (None, None),
            (Some(Ok(hp)), None) => (Some(hp), Some(0)),
            (Some(Ok(hp)), Some(Ok(sp))) => (Some(hp), Some(sp)),
            _ => return self.tell(e, MSG_HEAL_USAGE),
        };
        match (hp, sp) {
            (Some(hp), Some(sp)) => {
                self.change_vitals(e, hp, sp, 1);
                self.tell(e, MSG_HEAL_CHANGED);
            }
            _ => {
                let Some(status) = self.world.players.get(e).map(|info| info.status) else {
                    return;
                };
                self.heal_player(e, status.max_hp, status.max_sp);
                self.tell(e, MSG_HEALED);
            }
        }
    }
}
