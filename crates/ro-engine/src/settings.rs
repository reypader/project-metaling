//! Map-entry settings and the load confirm (spec `specs/map-entry-settings/spec.md`; catalog
//! `packets/settings.md`). Defaults only: nothing is saved yet.

use std::sync::Arc;

use ro_core::{ServerEvent, SkillInfo};

use crate::engine::MapInstance;
use crate::entity::Entity;

/// How a player entity came onto this map; decides the entry settings (rAthena `connect_new` /
/// `changemap`). A same-map warp keeps the entity, so it has no arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrival {
    /// From `CZ_ENTER`: the full settings group.
    Login,
    /// From another map: the config flags and reputation, no skill list or hotkeys.
    MapChange,
}

/// `ZC_CONFIG` types (rAthena `e_config_type`).
const CONFIG_CALL: u32 = 1;
const CONFIG_PET_AUTOFEED: u32 = 2;
const CONFIG_HOMUNCULUS_AUTOFEED: u32 = 3;
const CONFIG_DISABLE_SHOWCOSTUMES: u32 = 5;
/// The `ZC_CONFIG` flags sent on entry, in captured order.
const ENTRY_CONFIGS: [u32; 4] = [
    CONFIG_PET_AUTOFEED,
    CONFIG_CALL,
    CONFIG_HOMUNCULUS_AUTOFEED,
    CONFIG_DISABLE_SHOWCOSTUMES,
];

/// NV_BASIC (Basic Skill) at level 0, the one skill rAthena always lists for a fresh novice. Sent
/// to everyone until characters have a skill tree.
fn nv_basic() -> SkillInfo {
    SkillInfo {
        id: 1,
        inf: 0,
        level: 0,
        sp: 0,
        range: 0,
        name: Arc::from("NV_BASIC"),
        upgradable: true,
    }
}

impl MapInstance {
    /// Sends the settings group once per entry, after the unit entries: on a login the skill
    /// list, both hotkey banks and the status block, then on a login or cross-map arrival the
    /// party, equipment window and config flags and the reputation list. A same-map warp sends nothing.
    pub(crate) fn send_entry_settings(&mut self, e: Entity) {
        let Some(info) = self.world.players.get_mut(e) else {
            return;
        };
        let connect_new = std::mem::take(&mut info.connect_new);
        let changemap = std::mem::take(&mut info.changemap);
        if connect_new {
            self.emit_self(
                e,
                ServerEvent::SkillList {
                    skills: vec![nv_basic()],
                },
            );
            self.emit_self(e, ServerEvent::ShortcutKeyList { tab: 0 });
            self.emit_self(e, ServerEvent::ShortcutKeyList { tab: 1 });
            self.send_initial_status(e);
        }
        if changemap {
            self.emit_self(
                e,
                ServerEvent::PartyConfig {
                    deny_invites: false,
                },
            );
            self.emit_self(e, ServerEvent::EquipWindowConfig { show_equip: false });
            for config_type in ENTRY_CONFIGS {
                self.emit_self(
                    e,
                    ServerEvent::ConfigChanged {
                        config_type,
                        value: 0,
                    },
                );
            }
            self.emit_self(e, ServerEvent::ReputationList);
        }
    }
}
