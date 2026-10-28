//! The stat model in the engine (spec `specs/stats/spec.md` §2.1, §2.3; DESIGN.md §5, §9):
//! mapping a player into `ro_mechanics` inputs, the dirty set, the Stage 3 recalc pass and the
//! Stage 4 follow-ups.
//!
//! Flows never recalculate. They mark the player dirty (`mark_dirty`) and queue what rAthena
//! sends after its recalc (`queue_followup`). At the end of the tick each listed player gets, in
//! order: `SP_MAXWEIGHT` if it changed, the HP/SP clamp pushes, the diff, then its follow-ups.

use std::mem;

use ro_core::{CharId, CharacterProgress, ServerEvent};
use ro_db::{Job, WeaponType, DEFAULT_ASPD_BASE};
use ro_mechanics::{
    calc_status, BonusSlot, BonusTotals, GearInput, HandInput, JobRow, Primary, RefineInput,
    Status, StatusInputs, WeaponClass,
};
use tracing::warn;

use crate::engine::{MapInstance, SP_MAXWEIGHT};
use crate::entity::Entity;
use crate::equipment::{GearStatus, WeaponGear};
use crate::job::{class_flags, job_mapid, super_novice_bonus};
use crate::status::{battle_stats, couple, status_diff, SP_HP, SP_SP};

/// `PlayerInfo::stat_flags`: a recalc is due.
pub(crate) const DIRTY: u8 = 0x1;
/// The cached `GearStatus` must be rebuilt first (an equip change).
pub(crate) const STALE_GEAR: u8 = 0x2;
/// The player is in `MapInstance::stat_queue`.
const LISTED: u8 = 0x4;

/// rAthena's `MaxWeight` for a job without its own (catalog `db/job_stats.md`).
const DEFAULT_MAX_WEIGHT: u32 = 20_000;
/// The Super Novice rule's bonus to each primary stat.
const SUPER_NOVICE_STATS: i32 = 10;
const PRIMARY_SLOTS: [BonusSlot; 6] = [
    BonusSlot::Str,
    BonusSlot::Agi,
    BonusSlot::Vit,
    BonusSlot::Int,
    BonusSlot::Dex,
    BonusSlot::Luk,
];

/// What a flow queues for after the tick's recalc (spec §2.1 "Follow-up queue").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FollowUp {
    /// A ready packet for the player.
    ToSelf(ServerEvent),
    /// A packet for the player's area, the player included, resolved when it is sent.
    ToArea(ServerEvent),
    /// `0x0141` for STR..LUK `offset` from the snapshot this tick's recalc committed: a stat
    /// above 255 after its stat-up ack (which carries only 255).
    CoupleStat(u16),
    /// Every equip look to the area (`@job`, rAthena `pc_equiplookall`), each slot's view read
    /// when it is sent, so an equip later in the same tick isn't undone on screen.
    EquipLooks,
    /// Full HP and SP to the new maxima after a base level up: `SP_HP`/`SP_SP` for the parts
    /// it raised; nothing for a dead player.
    HealToMax,
    /// The `elvmax` re-check after a base level up (rAthena `pc_baselevelchanged`, the last
    /// step of `@blvl`): worn items above their max level come off.
    LevelGearCheck,
    /// Save the character now (`@job`; rAthena `chrif_save`).
    Save,
}

/// The formulas' weapon class of a weapon type (spec §2.2; plan M2 mapping notes).
fn weapon_class(weapon_type: WeaponType) -> WeaponClass {
    use WeaponType::*;
    match weapon_type {
        Fist => WeaponClass::Fist,
        Katar => WeaponClass::Katar,
        Bow => WeaponClass::Bow,
        Musical | Whip | Revolver | Rifle | Gatling | Shotgun | Grenade => WeaponClass::Ranged,
        _ => WeaponClass::Melee,
    }
}

fn aspd_base(job: Option<&Job>, weapon_type: WeaponType) -> u16 {
    job.map_or(DEFAULT_ASPD_BASE, |j| j.aspd_base(weapon_type))
}

fn hand_input(job: Option<&Job>, weapon: &WeaponGear) -> HandInput {
    HandInput {
        class: weapon_class(weapon.weapon_type),
        atk: weapon.atk,
        matk: weapon.matk,
        refine: RefineInput {
            bonus: weapon.refine.bonus,
            random_bonus: weapon.refine.random_bonus,
        },
        aspd_base: aspd_base(job, weapon.weapon_type),
    }
}

fn gear_input(job: Option<&Job>, gear: &GearStatus) -> GearInput {
    GearInput {
        right: gear.right.as_ref().map_or_else(
            || HandInput::bare(aspd_base(job, WeaponType::Fist)),
            |w| hand_input(job, w),
        ),
        left: gear.left.as_ref().map(|w| hand_input(job, w)),
        shield: gear.shield,
        item_def: i32::try_from(gear.hard_def).unwrap_or(i32::MAX),
        armor_refine: gear.armor_refine,
        // Item MDEF and gear VIT/INT come with item bonuses (spec §5).
        item_mdef: 0,
        vit: 0,
        int: 0,
        range: gear.range,
    }
}

fn job_row(job: Option<&Job>, job_level: u16, base_level: u16) -> JobRow {
    let Some(job) = job else {
        return JobRow {
            max_weight: DEFAULT_MAX_WEIGHT,
            ..JobRow::default()
        };
    };
    let bonus = job.bonus(job_level);
    JobRow {
        bonus: Primary {
            str: bonus.str.into(),
            agi: bonus.agi.into(),
            vit: bonus.vit.into(),
            int: bonus.int.into(),
            dex: bonus.dex.into(),
            luk: bonus.luk.into(),
        },
        base_hp: job.base_hp(base_level),
        base_sp: job.base_sp(base_level),
        max_weight: job.max_weight,
        shield_aspd: job.shield_aspd(),
    }
}

impl MapInstance {
    /// The character's job, levels, stats, points, exp and HP/SP (spec §2.7): what a save
    /// writes besides the inventory. Cheap: no inventory clone.
    pub fn progress_snapshot(&self, char_id: CharId) -> Option<CharacterProgress> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.players.get(e).map(|info| CharacterProgress {
            job_class: info.job,
            base_level: info.clevel,
            status: info.status,
        })
    }

    /// A change that can move `e`'s stats: the tick's pass recalculates it once. `gear` also
    /// rebuilds the cached gear status (equip changes).
    pub(crate) fn mark_dirty(&mut self, e: Entity, gear: bool) {
        let flags = if gear { DIRTY | STALE_GEAR } else { DIRTY };
        self.list_for_pass(e, flags);
    }

    /// Queues `followup` for after `e`'s recalc in this tick's pass.
    pub(crate) fn queue_followup(&mut self, e: Entity, followup: FollowUp) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        info.followups.push(followup);
        self.list_for_pass(e, 0);
    }

    fn list_for_pass(&mut self, e: Entity, flags: u8) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if info.stat_flags & LISTED == 0 {
            self.stat_queue.push(e);
        }
        info.stat_flags |= flags | LISTED;
    }

    /// `e`'s status from its current data and the cached gear status. `None` for a non-player.
    fn compute_status(&self, e: Entity) -> Option<Status> {
        let info = self.world.players.get(e)?;
        let job = self.jobs.get(info.job);
        let status = &info.status;
        let allocated = Primary {
            str: status.str.into(),
            agi: status.agi.into(),
            vit: status.vit.into(),
            int: status.int.into(),
            dex: status.dex.into(),
            luk: status.luk.into(),
        };
        let gear = gear_input(job, &info.gear);
        let mapid = job_mapid(info.job);
        // The base layer's only source until item bonuses and passives: the class extra.
        let mut base_bonus = BonusTotals::default();
        if super_novice_bonus(mapid, status.job_level) {
            for slot in PRIMARY_SLOTS {
                base_bonus.add(slot, SUPER_NOVICE_STATS);
            }
        }
        let none = BonusTotals::default();
        let inputs = StatusInputs {
            allocated,
            base_level: info.clevel,
            job: job_row(job, status.job_level, info.clevel),
            class: class_flags(mapid),
            gear: &gear,
            base_bonus: &base_bonus,
            battle_bonus: &none,
        };
        let mut computed = calc_status(&inputs, &self.battle);
        if job.is_none() {
            // No HP/SP table: keep the saved maxima.
            computed.max_hp = status.max_hp;
            computed.max_sp = status.max_sp;
        }
        Some(computed)
    }

    fn rebuild_gear(&mut self, e: Entity) {
        let gear = self
            .world
            .inventories
            .get(e)
            .map(|inventory| inventory.gear(&self.items).gear_status(&self.refine))
            .unwrap_or_default();
        if let Some(info) = self.world.players.get_mut(e) {
            info.gear = gear;
        }
    }

    /// Spawn and cross-map arrival: compute and store the status with nothing to diff against
    /// (spec §2.1). HP/SP above the new maxima are clamped silently; the login push sends them.
    pub(crate) fn init_status(&mut self, e: Entity) {
        self.rebuild_gear(e);
        let Some(computed) = self.compute_status(e) else {
            return;
        };
        if let Some(inventory) = self.world.inventories.get_mut(e) {
            inventory.set_max_weight(computed.max_weight);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        if self.jobs.get(info.job).is_none() {
            warn!(
                map = %self.map_name,
                char_id = ?info.char_id,
                job = info.job,
                "no job data: saved MaxHP/MaxSP kept, default MaxWeight and ASPD"
            );
        }
        let status = &mut info.status;
        status.max_hp = computed.max_hp;
        status.max_sp = computed.max_sp;
        status.hp = status.hp.min(computed.max_hp);
        status.sp = status.sp.min(computed.max_sp);
        info.allocated = [
            status.str, status.agi, status.vit, status.int, status.dex, status.luk,
        ];
        info.derived = computed;
    }

    /// Stages 3 and 4: every listed player in marking order gets its recalc (if dirty) and its
    /// follow-ups. Players a follow-up lists again wait for the next tick.
    pub(crate) fn run_stat_pass(&mut self) {
        let mut queue = mem::take(&mut self.stat_queue);
        for &e in &queue {
            self.settle_stats(e);
        }
        queue.clear();
        queue.append(&mut self.stat_queue);
        self.stat_queue = queue;
    }

    /// Runs what the pass owes `e` now: the recalc, then the follow-ups. Also used right before
    /// a cross-map warp takes the player away. Nothing for a player that already left.
    pub(crate) fn settle_stats(&mut self, e: Entity) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let flags = mem::take(&mut info.stat_flags);
        if flags & DIRTY != 0 {
            self.recalc(e, flags & STALE_GEAR != 0);
        }
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let mut followups = mem::take(&mut info.followups);
        for followup in followups.drain(..) {
            self.run_followup(e, followup);
        }
        if let Some(info) = self.world.players.get_mut(e) {
            // Keep the capacity; anything queued meanwhile stays for the next pass.
            followups.append(&mut info.followups);
            info.followups = followups;
        }
    }

    /// rAthena `status_calc_pc` at tick end: recompute, push `SP_MAXWEIGHT`, the clamps and the
    /// diff in the captured order (spec §2.3), store.
    fn recalc(&mut self, e: Entity, stale_gear: bool) {
        if stale_gear {
            self.rebuild_gear(e);
        }
        let Some(new) = self.compute_status(e) else {
            return;
        };
        let max_weight_changed = self.world.inventories.get_mut(e).is_some_and(|inventory| {
            let changed = inventory.max_weight() != new.max_weight;
            inventory.set_max_weight(new.max_weight);
            changed
        });
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let old = mem::replace(&mut info.derived, new);
        let status = &mut info.status;
        let old_vitals = (status.hp, status.sp);
        status.max_hp = new.max_hp;
        status.max_sp = new.max_sp;
        status.hp = status.hp.min(new.max_hp);
        status.sp = status.sp.min(new.max_sp);
        let new_vitals = (status.hp, status.sp);
        let allocated = [
            status.str, status.agi, status.vit, status.int, status.dex, status.luk,
        ];
        let old_allocated = mem::replace(&mut info.allocated, allocated);

        if max_weight_changed {
            self.emit_weight(e, SP_MAXWEIGHT, new.max_weight);
        }
        if new_vitals.0 != old_vitals.0 {
            self.emit_param(e, SP_HP, new_vitals.0);
        }
        if new_vitals.1 != old_vitals.1 {
            self.emit_param(e, SP_SP, new_vitals.1);
        }
        let allocated = (&old_allocated, &allocated);
        for event in status_diff(&old, &new, allocated, old_vitals, new_vitals) {
            self.emit_self(e, event);
        }
    }

    fn run_followup(&mut self, e: Entity, followup: FollowUp) {
        match followup {
            FollowUp::ToSelf(event) => self.emit_self(e, event),
            FollowUp::ToArea(event) => self.emit_aoi(e, false, event),
            FollowUp::CoupleStat(offset) => {
                let Some(info) = self.world.players.get(e) else {
                    return;
                };
                let i = usize::from(offset);
                let battle = battle_stats(&info.derived);
                let (Some(&allocated), Some(&battle)) = (info.allocated.get(i), battle.get(i))
                else {
                    return;
                };
                self.emit_self(e, couple(offset, allocated, battle));
            }
            FollowUp::EquipLooks => {
                for event in self.equip_look_events(e) {
                    self.emit_aoi(e, false, event);
                }
            }
            FollowUp::HealToMax => {
                // rAthena `status_percent_heal(100, 100)`: nothing for the dead; each part is
                // pushed only if it was below its max.
                let Some(status) = self.world.players.get_mut(e).map(|info| &mut info.status)
                else {
                    return;
                };
                if status.hp == 0 {
                    return;
                }
                let (hp_raised, sp_raised) = (status.hp < status.max_hp, status.sp < status.max_sp);
                status.hp = status.max_hp;
                status.sp = status.max_sp;
                let (hp, sp) = (status.hp, status.sp);
                if hp_raised {
                    self.emit_param(e, SP_HP, hp);
                }
                if sp_raised {
                    self.emit_param(e, SP_SP, sp);
                }
            }
            // The unequips mark the player dirty again: the diff they cause waits for the next
            // tick's pass.
            FollowUp::LevelGearCheck => self.unequip_over_level(e),
            FollowUp::Save => {
                if let Some(info) = self.world.players.get(e) {
                    self.save_requests.push(info.char_id);
                }
            }
        }
    }

    fn emit_param(&mut self, e: Entity, var_id: u16, value: u32) {
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id,
                value: i32::try_from(value).unwrap_or(i32::MAX),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use ro_core::{
        AccountId, BroadcastScope, CharId, CharacterSpawn, CharacterStatus, MapPosition,
        ScopedServerEvent, Sex,
    };
    use ro_db::{JobDb, MapGrid};

    use super::*;
    use crate::engine::Registries;

    const SP_MAXHP: u16 = 6;
    const SP_MAXSP: u16 = 8;

    fn char_id(n: u32) -> CharId {
        CharId(150_000 + n)
    }

    /// An instance with the shipped job data, and a Novice B11 J1 (MaxHP 90, MaxSP 21) at full
    /// HP/SP.
    fn novice_b11() -> (MapInstance, Entity) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.set_registries(Registries {
            jobs: Arc::new(JobDb::load_dir(&dir).expect("shipped job data")),
            ..Registries::default()
        });
        let spawn = CharacterSpawn {
            account_id: AccountId(2_000_001),
            char_id: char_id(1),
            name: "C1".into(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 11,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition {
                x: 10,
                y: 10,
                dir: 0,
            },
            speed: 150,
            status: CharacterStatus {
                hp: 1000,
                sp: 1000,
                ..CharacterStatus::default()
            },
            group_id: 0,
            inventory: Default::default(),
        };
        engine.spawn_character(spawn, 0).expect("spawn");
        let e = engine.world.entity_by_char(char_id(1)).expect("entity");
        (engine, e)
    }

    fn tick(engine: &mut MapInstance) -> Vec<ServerEvent> {
        let mut out: Vec<ScopedServerEvent> = Vec::new();
        engine.step_tick(100, &mut out).expect("tick");
        out.into_iter()
            .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(1)))
            .map(|e| e.event)
            .collect()
    }

    fn p(var_id: u16, value: i32) -> ServerEvent {
        ServerEvent::ParameterChanged { var_id, value }
    }

    fn set_level(engine: &mut MapInstance, e: Entity, level: u16) {
        engine.world.players.get_mut(e).expect("player").clevel = level;
    }

    #[test]
    fn spawn_clamps_hp_and_sp_to_the_computed_max() {
        let (engine, e) = novice_b11();
        let status = engine.world.players.get(e).expect("player").status;
        assert_eq!(
            (status.hp, status.max_hp, status.sp, status.max_sp),
            (90, 90, 21, 21)
        );
    }

    /// ENG-STAT-04: a lower MaxHP clamps HP: `SP_HP`/`SP_SP` before the diff, then the diff's
    /// MAXHP, MAXSP, HP, SP (captured double send, capture 1 lines 681-698).
    #[test]
    fn eng_stat_04_clamp_pushes_come_before_the_diff_and_again_in_it() {
        let (mut engine, e) = novice_b11();
        set_level(&mut engine, e, 1);
        engine.mark_dirty(e, false);
        let out = tick(&mut engine);
        assert_eq!(out[..2], [p(SP_HP, 40), p(SP_SP, 11)], "{out:?}");
        assert_eq!(
            out[out.len() - 4..],
            [p(SP_MAXHP, 40), p(SP_MAXSP, 11), p(SP_HP, 40), p(SP_SP, 11)],
            "{out:?}"
        );
        let status = engine.world.players.get(e).expect("player").status;
        assert_eq!((status.hp, status.max_hp), (40, 40));
    }

    /// ENG-STAT-10: two changes in one tick share one recalc; the follow-ups go out after the
    /// diff, in the order queued; nothing is owed the next tick.
    #[test]
    fn eng_stat_10_one_recalc_then_the_followups() {
        let (mut engine, e) = novice_b11();
        engine.world.players.get_mut(e).expect("player").status.luk = 3;
        engine.mark_dirty(e, false);
        engine.queue_followup(e, FollowUp::ToSelf(p(9, 46)));
        set_level(&mut engine, e, 12);
        engine.mark_dirty(e, true);
        engine.queue_followup(e, FollowUp::HealToMax);
        engine.queue_followup(e, FollowUp::Save);
        assert_eq!(engine.stat_queue.len(), 1, "listed once");
        let out = tick(&mut engine);
        let luk_pushes = out
            .iter()
            .filter(|ev| matches!(ev, ServerEvent::CoupleStatus { var_id: 18, .. }))
            .count();
        assert_eq!(luk_pushes, 1, "one diff: {out:?}");
        let max_hp = engine.world.players.get(e).expect("player").status.max_hp;
        let max_sp = engine.world.players.get(e).expect("player").status.max_sp;
        let tail = &out[out.len() - 3..];
        assert_eq!(
            tail,
            [
                p(9, 46),
                p(SP_HP, i32::try_from(max_hp).expect("small")),
                p(SP_SP, i32::try_from(max_sp).expect("small")),
            ],
            "{out:?}"
        );
        let mut saves = Vec::new();
        engine.drain_save_requests(&mut saves);
        assert_eq!(saves, vec![char_id(1)]);
        assert!(tick(&mut engine).is_empty(), "the queue is drained");
    }

    /// Review I-3: a stat reset whose battle value stays the same (the new job bonus makes up
    /// for it) still sends that stat's `0x0141`, so the window shows the new allocated base.
    #[test]
    fn a_reset_with_an_unchanged_battle_stat_still_sends_its_couple() {
        let (mut engine, e) = novice_b11();
        {
            let info = engine.world.players.get_mut(e).expect("player");
            info.status.str = 2;
        }
        engine.mark_dirty(e, false);
        tick(&mut engine);
        {
            // Novice job 10 adds +1 to every stat: STR 1 + 1 = the old battle 2.
            let info = engine.world.players.get_mut(e).expect("player");
            info.status.str = 1;
            info.status.job_level = 10;
        }
        engine.mark_dirty(e, false);
        let out = tick(&mut engine);
        assert!(
            out.contains(&ServerEvent::CoupleStatus {
                var_id: 13,
                base: 1,
                bonus: 1,
            }),
            "{out:?}"
        );
    }

    /// Review M-2 (catalog `mechanics/atcommands.md` "Heal pushes after the level-up"): the heal
    /// pushes only what it raised, and nothing for a dead player.
    #[test]
    fn heal_to_max_pushes_only_what_it_raised() {
        let (mut engine, e) = novice_b11();
        engine.world.players.get_mut(e).expect("player").status.hp = 50;
        engine.queue_followup(e, FollowUp::HealToMax);
        assert_eq!(tick(&mut engine), vec![p(SP_HP, 90)], "SP was full");
        engine.world.players.get_mut(e).expect("player").status.hp = 0;
        engine.queue_followup(e, FollowUp::HealToMax);
        assert!(tick(&mut engine).is_empty(), "dead: no heal");
        let hp = engine.world.players.get(e).expect("player").status.hp;
        assert_eq!(hp, 0);
    }

    /// Spec §2.1 / plan M3: a logout in the same tick drops what was owed.
    #[test]
    fn logout_drops_the_pending_recalc() {
        let (mut engine, e) = novice_b11();
        set_level(&mut engine, e, 1);
        engine.mark_dirty(e, false);
        engine.queue_followup(e, FollowUp::Save);
        engine.remove_character(char_id(1)).expect("removed");
        assert!(tick(&mut engine).is_empty());
        let mut saves = Vec::new();
        engine.drain_save_requests(&mut saves);
        assert!(saves.is_empty());
    }

    /// A cross-map warp flushes what the tick owes before the player leaves: the diff goes out
    /// first and the transfer carries the clamped HP.
    #[test]
    fn a_cross_map_warp_flushes_the_pending_recalc() {
        let (mut engine, e) = novice_b11();
        let warp = ro_db::NpcSpawn {
            name: "w".into(),
            unique_name: "w".into(),
            x: 10,
            y: 10,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: ro_db::NpcState::Normal,
            trigger: Some((0, 0)),
            duplicate_of: None,
            kind: ro_db::NpcKind::Warp(ro_db::WarpDest {
                map: "geffen".into(),
                x: 5,
                y: 5,
                trigger_while_hidden: false,
            }),
            source: "test:1".into(),
        };
        engine.spawn_npcs(&ro_db::MapNpcs { npcs: vec![warp] });
        set_level(&mut engine, e, 1);
        engine.mark_dirty(e, false);
        engine.push_intent(ro_core::SessionIntent::new(
            ro_core::SessionId(1),
            Some(AccountId(2_000_001)),
            Some(char_id(1)),
            ro_core::ClientIntent::NotifyActorInit,
        ));
        let out = tick(&mut engine);
        assert!(out.contains(&p(SP_MAXHP, 40)), "the diff went out: {out:?}");
        let mut transfers = Vec::new();
        engine.drain_transfers(&mut transfers);
        assert_eq!(transfers.len(), 1, "warped");
        assert_eq!(transfers[0].spawn.status.hp, 40);
    }

    #[test]
    fn weapon_classes_follow_the_plan_mapping() {
        assert_eq!(weapon_class(WeaponType::Fist), WeaponClass::Fist);
        assert_eq!(weapon_class(WeaponType::Dagger), WeaponClass::Melee);
        assert_eq!(weapon_class(WeaponType::TwoHandStaff), WeaponClass::Melee);
        assert_eq!(weapon_class(WeaponType::Katar), WeaponClass::Katar);
        assert_eq!(weapon_class(WeaponType::Bow), WeaponClass::Bow);
        for ranged in [
            WeaponType::Musical,
            WeaponType::Whip,
            WeaponType::Revolver,
            WeaponType::Rifle,
            WeaponType::Gatling,
            WeaponType::Shotgun,
            WeaponType::Grenade,
        ] {
            assert_eq!(weapon_class(ranged), WeaponClass::Ranged, "{ranged:?}");
        }
    }
}
