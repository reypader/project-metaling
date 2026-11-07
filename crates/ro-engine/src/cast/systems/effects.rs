//! Applying a definition's effects: heals, weapon-skill hits and the unit views they read.

use ro_core::ServerEvent;
use ro_mechanics::{calc_weapon_attack, AttackContext};
use ro_scripting::{Effect, HealShow, UnitView, Who};
use tracing::warn;

use crate::combat::attacker_view;
use crate::ecs::Entity;
use crate::map_instance::MapInstance;
use crate::stats::battle_stats;

/// `0x01de` type of every weapon-skill hit (`DMG_MULTI_HIT`, also on a miss).
const SKILL_DAMAGE_TYPE: u8 = 8;

impl MapInstance {
    /// Applies one resolved effect of `e`'s cast (or item use) on `target`.
    pub(crate) fn apply_effect(
        &mut self,
        e: Entity,
        target: Entity,
        skill_id: u16,
        level: u8,
        effect: Effect,
        now: u64,
    ) {
        match effect {
            Effect::SpendSp(amount) => self.spend_sp(e, amount),
            Effect::SpendHp(amount) => self.spend_hp(e, amount),
            Effect::SpendZeny(amount) => {
                let zeny = self.zeny(e).saturating_sub(amount);
                self.set_zeny(e, zeny);
            }
            Effect::ConsumeItem { item, .. } => {
                warn!(map = %self.map_name, skill = skill_id, item = item.0, "item costs are not applied yet");
            }
            Effect::Heal { to, hp, sp, show } => {
                let unit = match to {
                    Who::Caster => e,
                    Who::Target => target,
                };
                self.apply_heal(e, unit, skill_id, hp, sp, show);
            }
            Effect::WeaponDamage {
                ratio,
                hit_pct,
                div,
            } => self.weapon_skill_hit(e, target, skill_id, level, ratio, hit_pct, div, now),
        }
    }

    /// A heal from `caster` on `unit` (spec §2.5): `0x09cb` with the unclamped amount to the
    /// unit's area, then the player's own HP/SP pushes, or a mob's HP and its HP bar.
    fn apply_heal(
        &mut self,
        caster: Entity,
        unit: Entity,
        skill_id: u16,
        hp: u32,
        sp: u32,
        show: HealShow,
    ) {
        if show == HealShow::Skill {
            let (src, target) = (self.gid(caster), self.gid(unit));
            self.emit_aoi(
                unit,
                false,
                ServerEvent::SkillNoDamage {
                    skill_id,
                    value: i32::try_from(hp).unwrap_or(i32::MAX),
                    target,
                    src,
                    ok: true,
                },
            );
        }
        if self.world.players.contains(unit) {
            self.heal_player(unit, hp, sp);
        } else if let Some(mob) = self.world.mobs.get_mut(unit) {
            mob.hp = mob.hp.saturating_add(hp).min(mob.max_hp);
            if self.battle.monster_hp_bars_info {
                self.send_hp_bar(unit);
            }
        }
    }

    /// A weapon skill on a mob (spec §2.6): the shared damage roll with the skill context,
    /// `0x01de` to the target's area, and the hit at `now + amotion`. A miss queues nothing.
    #[allow(clippy::too_many_arguments)]
    fn weapon_skill_hit(
        &mut self,
        e: Entity,
        target: Entity,
        skill_id: u16,
        level: u8,
        ratio: i32,
        hit_pct: i32,
        div: i16,
        now: u64,
    ) {
        let (Some(info), Some(mob)) = (self.world.players.get(e), self.world.mobs.get(target))
        else {
            return;
        };
        let attacker = attacker_view(info, 0, false);
        let amotion = info.derived.amotion;
        let (src, dst) = (info.account_id.0, mob.gid);
        let dmotion = mob.stats.status.dmotion;
        let result = calc_weapon_attack(
            &attacker,
            &mob.stats.defender,
            &AttackContext::skill(ratio, hit_pct),
            &self.battle,
            |n| u32::try_from(self.rng.below(u64::from(n))).unwrap_or(0),
        );
        let hit = result.kind.is_hit();
        self.emit_aoi(
            target,
            false,
            ServerEvent::SkillDamage {
                skill_id,
                src,
                dst,
                tick: now as u32,
                src_speed: amotion,
                dmg_speed: if hit { dmotion } else { 0 },
                damage: result.damage,
                level: i16::from(level),
                div,
                damage_type: SKILL_DAMAGE_TYPE,
            },
        );
        if hit {
            self.queue_hit(e, target, result.damage, amotion, now);
        }
    }

    /// What a hook sees of a unit (DESIGN §2): battle stats, vitals, and for a player the MATK
    /// parts Heal reads (`status_matk + 5·SPL`, the right-hand weapon's MATK and level).
    pub(crate) fn unit_view(&self, u: Entity) -> UnitView {
        if let Some(info) = self.world.players.get(u) {
            let derived = &info.derived;
            let [str, agi, vit, int, dex, luk] = battle_stats(derived);
            let weapon = info.gear.right;
            return UnitView {
                base_level: info.clevel,
                job: info.job,
                str,
                agi,
                vit,
                int,
                dex,
                luk,
                hp: info.status.hp,
                max_hp: info.status.max_hp,
                sp: info.status.sp,
                max_sp: info.status.max_sp,
                matk_base: derived.status_matk + 5 * derived.trait_battle.spl,
                weapon_matk: weapon.map_or(0, |w| i32::try_from(w.matk).unwrap_or(i32::MAX)),
                weapon_level: weapon.map_or(0, |w| w.level),
            };
        }
        let Some(mob) = self.world.mobs.get(u) else {
            return UnitView::default();
        };
        let status = &mob.stats.status;
        let [str, agi, vit, int, dex, luk] = battle_stats(status);
        UnitView {
            base_level: mob.level,
            str,
            agi,
            vit,
            int,
            dex,
            luk,
            hp: mob.hp,
            max_hp: mob.max_hp,
            ..UnitView::default()
        }
    }
}
