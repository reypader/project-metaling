//! Map-entry settings and the load confirm (spec `specs/map-entry-settings/spec.md`; catalog
//! `packets/settings.md`). Defaults only: nothing is saved yet.

use std::sync::Arc;

use ro_core::{AchievementSummary, ServerEvent, SkillInfo};

use crate::engine::MapInstance;
use crate::entity::Entity;
use crate::wear::LOOK_WEAPON;

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

/// The system line a login gets after the friends list (rAthena sends its MOTD there).
const MOTD: &[u8] = b"Welcome to Metaling.";
/// Achievement exp for level 1 (`achievement_level_db` first row): the `0x0a24` "to next" value
/// of a character with no achievements.
const ACHIEVEMENT_FIRST_LEVEL_EXP: u32 = 18;
/// Renewal `natural_heal_weight_rate`: natural regeneration stops at this weight percentage.
const NATURAL_HEAL_WEIGHT_RATE: u32 = 70;
/// `0x099b` for a map with no mapflags: property `MAPPROPERTY_NOTHING`, flags USECART (bit 9)
/// and SUNMOONSTAR_MIRACLE (bit 10), which rAthena sets unless a mapflag forbids them.
const MAP_PROPERTY_NOTHING: u16 = 0;
const MAP_FLAGS_DEFAULT: u32 = 0x0600;

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
    /// What a login gets between `0x0201` and the client's first `0x007d`, in rAthena order
    /// (`pc_authok`, then the char-server replies): the MOTD line, `0x0091` to the character's
    /// own cell (the client reloads the map and sends `0x007d` twice), the status push, the mail
    /// icon, the achievement totals and the overweight percentage. The client sends `0x0447`
    /// only after a load that a `0x0091` started (spec §5).
    pub(crate) fn send_login_burst(&mut self, e: Entity) {
        self.emit_self(
            e,
            ServerEvent::SelfChat {
                text: MOTD.to_vec(),
            },
        );
        let (dest_x, dest_y) = self.cell(e);
        let dest_map = self.gat_name.to_string();
        self.emit_self(
            e,
            ServerEvent::PlayerMapMove {
                dest_map,
                dest_x,
                dest_y,
            },
        );
        self.send_login_status_push(e);
        self.emit_self(e, ServerEvent::UnreadMail { unread: false });
        self.emit_self(
            e,
            ServerEvent::AchievementUpdate {
                summary: AchievementSummary {
                    exp_to_next: ACHIEVEMENT_FIRST_LEVEL_EXP,
                    ..AchievementSummary::default()
                },
            },
        );
        self.emit_self(
            e,
            ServerEvent::OverweightPercent {
                percent: NATURAL_HEAL_WEIGHT_RATE,
            },
        );
    }

    /// `0x01d7` `LOOK_WEAPON` with both hands, first thing in a map entry. rAthena sends it to the
    /// area; only the player gets it here until observers need it (`DEFERRED.md`).
    pub(crate) fn send_weapon_look(&mut self, e: Entity) {
        let (value, value2) = self
            .world
            .players
            .get(e)
            .map_or((0, 0), |info| (info.looks.weapon, info.looks.shield));
        let gid = self.gid(e);
        self.emit_self(
            e,
            ServerEvent::SpriteChanged {
                gid,
                look_type: LOOK_WEAPON,
                value,
                value2,
            },
        );
    }

    /// `0x099b` for the current map. No mapflags exist yet, so every map gets the plain form.
    pub(crate) fn send_map_property(&mut self, e: Entity) {
        self.emit_self(
            e,
            ServerEvent::MapProperty {
                property: MAP_PROPERTY_NOTHING,
                flags: MAP_FLAGS_DEFAULT,
            },
        );
    }

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
