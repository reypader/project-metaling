//! Client intents: queueing and routing to the feature systems.

use ro_core::{ClientIntent, ServerEvent, SessionIntent};
use tracing::debug;

use super::MapInstance;
use crate::protocol_consts::{ACTION_ATTACK, ACTION_ATTACK_REPEAT, ACTION_SIT, ACTION_STAND};

impl MapInstance {
    pub(crate) fn handle_intent(&mut self, intent: SessionIntent, now_ms: u64) {
        let Some(e) = intent
            .char_id
            .and_then(|char_id| self.world.entity_by_char(char_id))
        else {
            debug!(map = %self.map_name, ?intent, "dropping intent for unknown character");
            return;
        };
        let visible = self.world.in_grid[e.index()];
        match intent.intent {
            ClientIntent::RequestTime { .. } => self.emit_self(
                e,
                ServerEvent::TimeSynchronized {
                    server_tick: now_ms as u32,
                },
            ),
            ClientIntent::NotifyActorInit => self.actor_init(e, now_ms),
            _ if !visible => {
                debug!(map = %self.map_name, "ignoring gameplay intent before actor init");
            }
            ClientIntent::RequestMove { dest_x, dest_y } => {
                let slot = &mut self.world.pending_move[e.index()];
                if slot.is_none() {
                    self.moves.push(e);
                }
                *slot = Some((dest_x, dest_y));
            }
            ClientIntent::ChangeDirection { head_dir, body_dir } => {
                let i = e.index();
                self.world.head_dir[i] = head_dir;
                self.world.positions[i].dir = body_dir;
                let gid = self.gid(e);
                self.emit_aoi(
                    e,
                    true,
                    ServerEvent::DirectionChanged {
                        gid,
                        head_dir,
                        body_dir,
                    },
                );
            }
            ClientIntent::RequestAction {
                target_id,
                action_type,
            } => match action_type {
                ACTION_ATTACK | ACTION_ATTACK_REPEAT => {
                    self.request_attack(e, target_id, action_type == ACTION_ATTACK_REPEAT, now_ms)
                }
                other => {
                    self.stop_walking(e);
                    self.drop_attack(e);
                    match other {
                        ACTION_SIT => self.sit(e),
                        ACTION_STAND => self.stand(e),
                        other => debug!(map = %self.map_name, action = other, "ignoring action"),
                    }
                }
            },
            ClientIntent::CancelAttack => self.cancel_attack(e),
            ClientIntent::RequestEntityName { entity_id } => self.name_request(e, entity_id),
            ClientIntent::PickUpItem { floor_id } => self.pick_up(e, floor_id, now_ms),
            ClientIntent::Chat { text } => self.chat(e, &text),
            ClientIntent::DropItem { index, amount } => self.drop_item(e, index, amount, now_ms),
            ClientIntent::EquipItem { index, position } => self.equip_item(e, index, position),
            ClientIntent::UnequipItem { index } => self.unequip_item(e, index),
            ClientIntent::UnequipAll { location } => self.unequip_all(e, location),
            ClientIntent::RaiseStat { var, amount } => self.raise_stat(e, var, amount),
            ClientIntent::RaiseTrait { var, amount } => self.raise_trait(e, var, amount),
            ClientIntent::UpgradeSkill { id } => self.upgrade_skill(e, id),
            ClientIntent::UseSkill {
                level,
                skill_id,
                target,
            } => self.use_skill(e, skill_id, level, target, now_ms),
            ClientIntent::UseItem { index } => self.use_item(e, index, now_ms),
            ClientIntent::BlockingPlayCancel => self.emit_self(e, ServerEvent::LoadConfirmed),
            other => debug!(map = %self.map_name, intent = ?other, "intent not handled by engine"),
        }
    }

    /// Queues an intent for the next `step_tick`. `EnterMap` is not accepted here.
    pub fn push_intent(&mut self, intent: SessionIntent) {
        if matches!(intent.intent, ClientIntent::EnterMap { .. }) {
            debug!(map = %self.map_name, "EnterMap is handled by spawn_character, not intents");
            return;
        }
        self.intents.push(intent);
    }
}
