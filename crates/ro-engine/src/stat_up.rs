//! Stat-up, `0x00bb` (spec `specs/stats/spec.md` §2.4; catalog `packets/status.md` "Stat-up
//! request and acknowledgement").
//!
//! A raise changes the allocated stat and the points at once and marks the player dirty. The
//! tick's recalc pushes the diff, and then the queued `0x00be`, `SP_STATUSPOINT` and ack go out,
//! in `pc_statusup`'s order. A failure sends only the `ok = 0` ack, at once.

use ro_core::{CharacterStatus, ServerEvent};
use ro_mechanics::{next_stat_cost, raise_stat};

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::job::{class_flags, job_mapid};
use crate::stats::FollowUp;
use crate::status::{param, SP_LUK, SP_STATUSPOINT, SP_STR, SP_USTR};

/// The ack carries the stat in one byte; a higher stat gets a trailing `0x0141`.
const ACK_MAX_VALUE: u16 = 255;

/// The allocated stat at STR..LUK `offset`.
fn stat_mut(status: &mut CharacterStatus, offset: u16) -> Option<&mut u16> {
    match offset {
        0 => Some(&mut status.str),
        1 => Some(&mut status.agi),
        2 => Some(&mut status.vit),
        3 => Some(&mut status.int),
        4 => Some(&mut status.dex),
        5 => Some(&mut status.luk),
        _ => None,
    }
}

impl MapInstance {
    /// `CZ_STATUS_CHANGE`: raise stat `var` by up to `amount`, as far as the points and the
    /// job's cap allow. Refused while dead (our ruling; rAthena doesn't check), not while
    /// sitting.
    pub(crate) fn raise_stat(&mut self, e: Entity, var: u16, amount: u8) {
        if !(SP_STR..=SP_LUK).contains(&var) || amount == 0 || self.is_dead(e) {
            return self.refuse_raise(e, var);
        }
        let offset = var - SP_STR;
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let cap = self.battle.max_stat(&class_flags(job_mapid(info.job)));
        let points = info.status.status_point;
        let Some(stat) = stat_mut(&mut info.status, offset) else {
            return;
        };
        let raise = raise_stat(*stat, amount.into(), points.into(), cap);
        let Ok(cost) = u16::try_from(raise.cost) else {
            return self.refuse_raise(e, var);
        };
        if raise.amount == 0 {
            return self.refuse_raise(e, var);
        }
        *stat += raise.amount;
        let new = *stat;
        // `raise_stat` never spends more than `points`.
        info.status.status_point = points.saturating_sub(cost);
        let points = info.status.status_point;

        self.mark_dirty(e, false);
        for event in [
            ServerEvent::StatusCost {
                var_id: SP_USTR + offset,
                value: next_stat_cost(new, cap),
            },
            param(SP_STATUSPOINT, points.into()),
            ServerEvent::StatusChangeAck {
                var_id: var,
                ok: true,
                value: u8::try_from(new.min(ACK_MAX_VALUE)).unwrap_or(u8::MAX),
            },
        ] {
            self.queue_followup(e, FollowUp::ToSelf(event));
        }
        if new > ACK_MAX_VALUE {
            self.queue_followup(e, FollowUp::CoupleStat(offset));
        }
    }

    fn refuse_raise(&mut self, e: Entity, var: u16) {
        self.emit_self(
            e,
            ServerEvent::StatusChangeAck {
                var_id: var,
                ok: false,
                value: 0,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ro_core::ClientIntent;
    use ro_core::{
        AccountId, BroadcastScope, CharId, CharacterSpawn, MapPosition, ScopedServerEvent,
        SessionId, SessionIntent, Sex,
    };
    use ro_db::MapGrid;
    use ro_mechanics::BattleConfig;

    use super::*;
    use crate::engine::Registries;

    /// `dead_sit` of a dead unit.
    const DEAD: u8 = 1;
    const CHAR: CharId = CharId(150_001);

    /// A Novice with no job data (so the battle config alone sets the cap) and `status`.
    fn novice(battle: BattleConfig, status: CharacterStatus) -> (MapInstance, Entity) {
        let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.set_registries(Registries {
            battle: Arc::new(battle),
            ..Registries::default()
        });
        let spawn = CharacterSpawn {
            account_id: AccountId(2_000_001),
            char_id: CHAR,
            name: "C1".into(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition {
                x: 10,
                y: 10,
                dir: 0,
            },
            speed: 150,
            status,
            group_id: 0,
            inventory: Default::default(),
        };
        engine.spawn_character(spawn, 0).expect("spawn");
        send(&mut engine, &[ClientIntent::NotifyActorInit]);
        let e = engine.world.entity_by_char(CHAR).expect("entity");
        (engine, e)
    }

    /// One tick with `intents`; what the player got.
    fn send(engine: &mut MapInstance, intents: &[ClientIntent]) -> Vec<ServerEvent> {
        for intent in intents {
            engine.push_intent(SessionIntent::new(
                SessionId(1),
                Some(AccountId(2_000_001)),
                Some(CHAR),
                intent.clone(),
            ));
        }
        let mut out: Vec<ScopedServerEvent> = Vec::new();
        engine.step_tick(100, &mut out).expect("tick");
        out.into_iter()
            .filter(|e| e.scope == BroadcastScope::SelfOnly(CHAR))
            .map(|e| e.event)
            .collect()
    }

    fn raise(engine: &mut MapInstance, var: u16, amount: u8) -> Vec<ServerEvent> {
        send(engine, &[ClientIntent::RaiseStat { var, amount }])
    }

    /// Spec §2.4: refused while dead (our ruling), with nothing changed.
    #[test]
    fn eng_stat_05_refused_while_dead() {
        let (mut engine, e) = novice(BattleConfig::default(), CharacterStatus::default());
        engine.world.dead_sit[e.index()] = DEAD;
        let out = raise(&mut engine, SP_STR, 1);
        assert_eq!(
            out,
            vec![ServerEvent::StatusChangeAck {
                var_id: SP_STR,
                ok: false,
                value: 0
            }]
        );
        let status = engine.world.players.get(e).expect("player").status;
        assert_eq!((status.str, status.status_point), (1, 48));
    }

    /// Spec §2.4: a stat above 255 acks 255, then its real value follows as `0x0141` from the
    /// committed snapshot (only reachable with a raised `max_parameter`).
    #[test]
    fn eng_stat_05_a_stat_above_255_gets_a_trailing_couple() {
        let battle = BattleConfig {
            max_parameter: 300,
            ..BattleConfig::default()
        };
        let status = CharacterStatus {
            str: 255,
            status_point: 1000,
            ..CharacterStatus::default()
        };
        let (mut engine, _) = novice(battle, status);
        let out = raise(&mut engine, SP_STR, 2);
        assert_eq!(
            out[out.len() - 2..],
            [
                ServerEvent::StatusChangeAck {
                    var_id: SP_STR,
                    ok: true,
                    value: 255
                },
                ServerEvent::CoupleStatus {
                    var_id: SP_STR.into(),
                    base: 257,
                    bonus: 0
                },
            ],
            "{out:?}"
        );
    }

    /// Spec §2.4: two raises in one tick share one recalc; each one's follow-ups carry its own
    /// values.
    #[test]
    fn eng_stat_05_two_raises_in_one_tick() {
        let (mut engine, _) = novice(BattleConfig::default(), CharacterStatus::default());
        let up = ClientIntent::RaiseStat {
            var: SP_STR,
            amount: 1,
        };
        let out = send(&mut engine, &[up.clone(), up]);
        let couples = out
            .iter()
            .filter(|ev| matches!(ev, ServerEvent::CoupleStatus { .. }))
            .count();
        assert_eq!(couples, 1, "one diff: {out:?}");
        let acks: Vec<_> = out
            .iter()
            .filter_map(|ev| match ev {
                ServerEvent::StatusChangeAck { value, .. } => Some(*value),
                ServerEvent::ParameterChanged {
                    var_id: SP_STATUSPOINT,
                    value,
                } => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect();
        assert_eq!(acks, vec![46, 2, 44, 3], "points then ack, per raise");
    }
}
