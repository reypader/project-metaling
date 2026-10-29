//! Event encoder connecting domain ServerEvent variants to outgoing packet encoders.

use crate::{EventEncoder, PacketCodecError, PacketOpcode};
use bytes::{BufMut, Bytes, BytesMut};
use ro_core::ServerEvent;

use super::char::{
    encode_hc_accept_enter, encode_hc_accept_enter2, encode_hc_accept_makechar,
    encode_hc_ack_charinfo_per_page, encode_hc_delete_char3_cancel,
    encode_hc_delete_char3_reserved, encode_hc_notify_zonesvr, encode_hc_refuse_enter,
    encode_hc_refuse_makechar,
};
use super::equip::{
    encode_zc_ack_takeoff_equip_all, encode_zc_action_failure, encode_zc_equip_arrow,
    encode_zc_msg, encode_zc_req_takeoff_equip_ack, encode_zc_req_wear_equip_ack,
};
use super::item::{
    encode_zc_equipment_itemlist, encode_zc_equipswitch_list_empty, encode_zc_inventory_end,
    encode_zc_inventory_start, encode_zc_item_disappear, encode_zc_item_entry,
    encode_zc_item_fall_entry, encode_zc_item_pickup_ack, encode_zc_item_throw_ack,
    encode_zc_normal_itemlist, encode_zc_notify_chat, encode_zc_notify_playerchat,
};
use super::login::{encode_ac_accept_login, encode_ac_refuse_login, CharServerEntry};
use super::map::{
    encode_zc_accept_enter, encode_zc_ack_req_disconnect, encode_zc_ack_reqnameall,
    encode_zc_ack_reqnameall_npc, encode_zc_aid, encode_zc_change_direction,
    encode_zc_inventory_expansion_info, encode_zc_notify_act, encode_zc_notify_moveentry11,
    encode_zc_notify_newentry11, encode_zc_notify_playermove, encode_zc_notify_standentry11,
    encode_zc_notify_time, encode_zc_notify_vanish, encode_zc_npcack_mapmove,
    encode_zc_par_change_i32, encode_zc_restart_ack, encode_zc_sprite_change2, encode_zc_stopmove,
};
use super::settings::{
    encode_zc_ach_update_totals, encode_zc_config, encode_zc_config_notify,
    encode_zc_friends_list_empty, encode_zc_load_confirm, encode_zc_notify_mapproperty2,
    encode_zc_notify_unreadmail, encode_zc_overweight_percent, encode_zc_party_config,
    encode_zc_repute_info_default, encode_zc_shortcut_key_list_empty, encode_zc_skillinfo_list,
};
use super::status::{
    encode_zc_attack_range, encode_zc_couplestatus, encode_zc_longlongpar_change,
    encode_zc_msg_state_change3, encode_zc_notify_effect, encode_zc_status,
    encode_zc_status_change, encode_zc_status_change_ack,
};

/// `ZC_NOTIFY_ACT` types for sit down / stand up (`e_damage_type`).
const ACT_SIT_DOWN: u8 = 2;
const ACT_STAND_UP: u8 = 3;
/// `DMG_PICKUP_ITEM`.
const ACT_PICKUP_ITEM: u8 = 1;

/// Default event encoder connecting domain `ServerEvent` variants to Phase 0 packet serializers.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase0EventEncoder;

impl EventEncoder for Phase0EventEncoder {
    fn encode_event(&self, event: &ServerEvent, packetver: u32) -> Result<Bytes, PacketCodecError> {
        ModernEventEncoder.encode_event(event, packetver)
    }
}

/// Modern event encoder connecting domain `ServerEvent` variants to modern packet serializers.
#[derive(Debug, Default, Clone, Copy)]
pub struct ModernEventEncoder;

impl EventEncoder for ModernEventEncoder {
    fn encode_event(
        &self,
        event: &ServerEvent,
        _packetver: u32,
    ) -> Result<Bytes, PacketCodecError> {
        match event {
            ServerEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                char_servers,
            } => {
                let entries: Vec<CharServerEntry> =
                    char_servers.iter().map(CharServerEntry::from).collect();
                encode_ac_accept_login(
                    *login_id1,
                    *account_id,
                    *login_id2,
                    [0, 0, 0, 0],
                    "",
                    *sex,
                    &entries,
                )
            }
            ServerEvent::LoginRefused { error_code } => {
                Ok(encode_ac_refuse_login(*error_code as u32, ""))
            }
            ServerEvent::CharacterListLoaded {
                account_id,
                total_slots,
                characters,
            } => {
                let mut buf = BytesMut::new();
                buf.extend_from_slice(&encode_hc_accept_enter2(
                    *total_slots,
                    0,
                    *total_slots,
                    *total_slots,
                    *total_slots,
                ));
                buf.extend_from_slice(&encode_hc_accept_enter(*total_slots, 0, 0, characters)?);
                let pages = ((*total_slots as u32) / 3).max(1);
                buf.put_u16_le(PacketOpcode::HC_CHARLIST_NOTIFY.raw());
                buf.put_u32_le(pages);
                buf.put_u16_le(PacketOpcode::HC_BLOCK_CHARACTER.raw());
                buf.put_u16_le(4);
                buf.put_u16_le(PacketOpcode::HC_SECOND_PASSWD_LOGIN.raw());
                buf.put_u32_le(0);
                buf.put_u32_le(account_id.0);
                buf.put_u16_le(0);
                Ok(buf.freeze())
            }
            ServerEvent::CharacterListRefused { error_code } => {
                Ok(encode_hc_refuse_enter(*error_code))
            }
            ServerEvent::CharacterPageLoaded { characters } => {
                encode_hc_ack_charinfo_per_page(characters)
            }
            ServerEvent::CharacterCreated { character } => Ok(encode_hc_accept_makechar(character)),
            ServerEvent::CharacterCreateRefused { error_code } => {
                Ok(encode_hc_refuse_makechar(*error_code))
            }
            ServerEvent::CharacterDeleteReserved {
                char_id,
                result,
                delete_delay_seconds,
            } => Ok(encode_hc_delete_char3_reserved(
                *char_id,
                *result,
                *delete_delay_seconds,
            )),
            ServerEvent::CharacterDeleteCancelled { char_id, result } => {
                Ok(encode_hc_delete_char3_cancel(*char_id, *result))
            }
            ServerEvent::CharacterSelected {
                char_id,
                map_name,
                map_host,
                map_port,
            } => Ok(encode_hc_notify_zonesvr(
                *char_id, map_name, *map_host, *map_port,
            )),
            ServerEvent::AcceptEnter {
                account_id,
                server_tick,
                pos,
            } => {
                let mut buf = BytesMut::with_capacity(23);
                buf.extend_from_slice(&encode_zc_aid(*account_id));
                buf.extend_from_slice(&encode_zc_inventory_expansion_info(0));
                buf.extend_from_slice(&encode_zc_accept_enter(*server_tick, *pos, 5, 5));
                Ok(buf.freeze())
            }
            ServerEvent::Aid { account_id } => Ok(encode_zc_aid(*account_id)),
            ServerEvent::InventoryExpansionInfo { expansion_size } => {
                Ok(encode_zc_inventory_expansion_info(*expansion_size))
            }
            ServerEvent::TimeSynchronized { server_tick } => {
                Ok(encode_zc_notify_time(*server_tick))
            }
            ServerEvent::PlayerMoved {
                start,
                dest,
                move_start_tick,
            } => Ok(encode_zc_notify_playermove(
                start.0,
                start.1,
                dest.0,
                dest.1,
                *move_start_tick,
            )),
            ServerEvent::UnitWalking {
                unit,
                start,
                dest,
                move_start_tick,
            } => Ok(encode_zc_notify_moveentry11(
                unit,
                *start,
                *dest,
                *move_start_tick,
            )),
            ServerEvent::UnitStanding { unit } => Ok(encode_zc_notify_standentry11(unit)),
            ServerEvent::UnitSpawned { unit } => Ok(encode_zc_notify_newentry11(unit)),
            ServerEvent::UnitVanished { gid, vanish_type } => {
                Ok(encode_zc_notify_vanish(*gid, *vanish_type))
            }
            ServerEvent::UnitStopped { gid, x, y } => Ok(encode_zc_stopmove(*gid, *x, *y)),
            ServerEvent::DirectionChanged {
                gid,
                head_dir,
                body_dir,
            } => Ok(encode_zc_change_direction(*gid, *head_dir, *body_dir)),
            ServerEvent::AckEntityName {
                gid,
                name,
                party_name,
                guild_name,
                position_name,
                title_id,
            } => Ok(encode_zc_ack_reqnameall(
                *gid,
                name,
                party_name,
                guild_name,
                position_name,
                *title_id,
            )),
            ServerEvent::AckNpcName { gid, name } => Ok(encode_zc_ack_reqnameall_npc(*gid, name)),
            ServerEvent::PlayerMapMove {
                dest_map,
                dest_x,
                dest_y,
            } => Ok(encode_zc_npcack_mapmove(dest_map, *dest_x, *dest_y)),
            ServerEvent::ParameterChanged { var_id, value } => {
                Ok(encode_zc_par_change_i32(*var_id, *value))
            }
            ServerEvent::EntityAction {
                source_id,
                target_id,
                action_type,
                action_tick,
            } => {
                if *action_type <= 8 {
                    if *action_type == ACT_PICKUP_ITEM {
                        // rAthena's pickup animation sets only src, target and type (captured).
                        Ok(encode_zc_notify_act(
                            *source_id,
                            *target_id,
                            0,
                            0,
                            0,
                            0,
                            0,
                            *action_type,
                            0,
                        ))
                    } else if matches!(*action_type, ACT_SIT_DOWN | ACT_STAND_UP) {
                        // rAthena zero-initializes sit/stand and sets only src and type.
                        Ok(encode_zc_notify_act(
                            *source_id,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            *action_type,
                            0,
                        ))
                    } else {
                        Ok(encode_zc_notify_act(
                            *source_id,
                            *target_id,
                            *action_tick,
                            0,
                            0,
                            0,
                            1,
                            *action_type,
                            0,
                        ))
                    }
                } else {
                    Err(PacketCodecError::UnsupportedEvent(format!(
                        "Action type {action_type} is not implemented"
                    )))
                }
            }
            ServerEvent::RestartAck { result } => Ok(encode_zc_restart_ack(*result)),
            ServerEvent::DisconnectAck { result } => Ok(encode_zc_ack_req_disconnect(*result)),
            ServerEvent::InventoryStart => Ok(encode_zc_inventory_start()),
            ServerEvent::InventoryNormalItems { items } => Ok(encode_zc_normal_itemlist(items)),
            ServerEvent::InventoryEquipItems { items } => Ok(encode_zc_equipment_itemlist(items)),
            ServerEvent::InventoryEnd => Ok(encode_zc_inventory_end()),
            ServerEvent::EquipSwitchList => Ok(encode_zc_equipswitch_list_empty()),
            ServerEvent::ItemAdded { item } => Ok(encode_zc_item_pickup_ack(Some(item), 0)),
            ServerEvent::ItemAddFailed { result } => Ok(encode_zc_item_pickup_ack(None, *result)),
            ServerEvent::ItemThrown { index, amount } => {
                Ok(encode_zc_item_throw_ack(*index, *amount))
            }
            ServerEvent::FloorItemDropped { item } => Ok(encode_zc_item_fall_entry(item)),
            ServerEvent::FloorItemEntered { item } => Ok(encode_zc_item_entry(item)),
            ServerEvent::FloorItemVanished { id } => Ok(encode_zc_item_disappear(*id)),
            ServerEvent::PublicChat { gid, text } => Ok(encode_zc_notify_chat(*gid, text)),
            ServerEvent::SelfChat { text } => Ok(encode_zc_notify_playerchat(text)),
            ServerEvent::FriendsList => Ok(encode_zc_friends_list_empty()),
            ServerEvent::SkillList { skills } => Ok(encode_zc_skillinfo_list(skills)),
            ServerEvent::LongLongParameterChanged { var_id, value } => {
                Ok(encode_zc_longlongpar_change(*var_id, *value))
            }
            ServerEvent::StatusWindow { status } => Ok(encode_zc_status(status)),
            ServerEvent::CoupleStatus {
                var_id,
                base,
                bonus,
            } => Ok(encode_zc_couplestatus(*var_id, *base, *bonus)),
            ServerEvent::AttackRange { range } => Ok(encode_zc_attack_range(*range)),
            ServerEvent::StatusCost { var_id, value } => {
                Ok(encode_zc_status_change(*var_id, *value))
            }
            ServerEvent::ShortcutKeyList { tab } => Ok(encode_zc_shortcut_key_list_empty(*tab)),
            ServerEvent::PartyConfig { deny_invites } => Ok(encode_zc_party_config(*deny_invites)),
            ServerEvent::EquipWindowConfig { show_equip } => {
                Ok(encode_zc_config_notify(*show_equip))
            }
            ServerEvent::ConfigChanged { config_type, value } => {
                Ok(encode_zc_config(*config_type, *value))
            }
            ServerEvent::ReputationList => Ok(encode_zc_repute_info_default()),
            ServerEvent::LoadConfirmed => Ok(encode_zc_load_confirm()),
            ServerEvent::SpriteChanged {
                gid,
                look_type,
                value,
                value2,
            } => Ok(encode_zc_sprite_change2(*gid, *look_type, *value, *value2)),
            ServerEvent::MapProperty { property, flags } => {
                Ok(encode_zc_notify_mapproperty2(*property, *flags))
            }
            ServerEvent::UnreadMail { unread } => Ok(encode_zc_notify_unreadmail(*unread)),
            ServerEvent::AchievementUpdate { summary } => Ok(encode_zc_ach_update_totals(summary)),
            ServerEvent::OverweightPercent { percent } => {
                Ok(encode_zc_overweight_percent(*percent))
            }
            ServerEvent::EquipAck {
                index,
                location,
                view,
                result,
            } => Ok(encode_zc_req_wear_equip_ack(
                *index, *location, *view, *result,
            )),
            ServerEvent::UnequipAck {
                index,
                location,
                failed,
            } => Ok(encode_zc_req_takeoff_equip_ack(*index, *location, *failed)),
            ServerEvent::UnequipAllAck { failed } => Ok(encode_zc_ack_takeoff_equip_all(*failed)),
            ServerEvent::AmmoEquipped { index } => Ok(encode_zc_equip_arrow(*index)),
            ServerEvent::ActionFailure { failure_type } => {
                Ok(encode_zc_action_failure(*failure_type))
            }
            ServerEvent::ClientMessage { msg_id } => Ok(encode_zc_msg(*msg_id)),
            ServerEvent::StatusIconStarted {
                efst,
                gid,
                state,
                total_ms,
                remain_ms,
                values,
            } => Ok(encode_zc_msg_state_change3(
                *efst, *gid, *state, *total_ms, *remain_ms, *values,
            )),
            ServerEvent::StatusChangeAck { var_id, ok, value } => {
                Ok(encode_zc_status_change_ack(*var_id, *ok, *value))
            }
            ServerEvent::SpecialEffect { gid, effect } => {
                Ok(encode_zc_notify_effect(*gid, *effect))
            }
        }
    }
}

/// Rathena-compatible alias for modern event encoder.
pub type RathenaEventEncoder = ModernEventEncoder;

/// Returns whether the specified opcode is an outbound server-to-client packet currently encoded by this crate.
pub fn is_implemented_outbound_opcode(opcode: PacketOpcode, _packetver: u32) -> bool {
    matches!(
        opcode,
        PacketOpcode::HC_ACCEPT_ENTER
            | PacketOpcode::HC_REFUSE_ENTER
            | PacketOpcode::ZC_NOTIFY_TIME
            | PacketOpcode::ZC_NOTIFY_VANISH
            | PacketOpcode::ZC_NOTIFY_PLAYERMOVE
            | PacketOpcode::ZC_PAR_CHANGE
            | PacketOpcode::ZC_ACCEPT_ENTER
            | PacketOpcode::HC_ACCEPT_ENTER2
            | PacketOpcode::AC_REFUSE_LOGIN
            | PacketOpcode::ZC_NOTIFY_ACT
            | PacketOpcode::HC_ACK_CHARINFO_PER_PAGE
            | PacketOpcode::HC_ACCEPT_MAKECHAR
            | PacketOpcode::HC_REFUSE_MAKECHAR
            | PacketOpcode::HC_DELETE_CHAR3_RESERVED
            | PacketOpcode::HC_DELETE_CHAR3_CANCEL
            | PacketOpcode::ZC_RESTART_ACK
            | PacketOpcode::ZC_ACK_REQ_DISCONNECT
            | PacketOpcode::HC_CHARLIST_NOTIFY
            | PacketOpcode::HC_BLOCK_CHARACTER
            | PacketOpcode::HC_SECOND_PASSWD_LOGIN
            | PacketOpcode::ZC_NOTIFY_MOVEENTRY11
            | PacketOpcode::ZC_STOPMOVE
            | PacketOpcode::ZC_CHANGE_DIRECTION
            | PacketOpcode::ZC_ACK_REQNAMEALL
            | PacketOpcode::ZC_ACK_REQNAMEALL_NPC
            | PacketOpcode::ZC_NPCACK_MAPMOVE
            | PacketOpcode::ZC_NOTIFY_NEWENTRY11
            | PacketOpcode::ZC_NOTIFY_STANDENTRY11
            | PacketOpcode::AC_ACCEPT_LOGIN
            | PacketOpcode::HC_NOTIFY_ZONESVR
            | PacketOpcode::ZC_AID
            | PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
            | PacketOpcode::ZC_INVENTORY_START
            | PacketOpcode::ZC_NORMAL_ITEMLIST
            | PacketOpcode::ZC_EQUIPMENT_ITEMLIST
            | PacketOpcode::ZC_INVENTORY_END
            | PacketOpcode::ZC_EQUIPSWITCH_LIST
            | PacketOpcode::ZC_ITEM_PICKUP_ACK
            | PacketOpcode::ZC_ITEM_THROW_ACK
            | PacketOpcode::ZC_ITEM_FALL_ENTRY5
            | PacketOpcode::ZC_ITEM_ENTRY
            | PacketOpcode::ZC_ITEM_DISAPPEAR
            | PacketOpcode::ZC_NOTIFY_CHAT
            | PacketOpcode::ZC_NOTIFY_PLAYERCHAT
            | PacketOpcode::ZC_FRIENDS_LIST
            | PacketOpcode::ZC_SKILLINFO_LIST
            | PacketOpcode::ZC_SHORTCUT_KEY_LIST_V4
            | PacketOpcode::ZC_PARTY_CONFIG
            | PacketOpcode::ZC_CONFIG_NOTIFY
            | PacketOpcode::ZC_CONFIG
            | PacketOpcode::ZC_REPUTE_INFO
            | PacketOpcode::ZC_LOAD_CONFIRM
            | PacketOpcode::ZC_LONGLONGPAR_CHANGE
            | PacketOpcode::ZC_STATUS
            | PacketOpcode::ZC_COUPLESTATUS
            | PacketOpcode::ZC_ATTACK_RANGE
            | PacketOpcode::ZC_STATUS_CHANGE
            | PacketOpcode::ZC_STATUS_CHANGE_ACK
            | PacketOpcode::ZC_NOTIFY_EFFECT
            | PacketOpcode::ZC_SPRITE_CHANGE2
            | PacketOpcode::ZC_NOTIFY_MAPPROPERTY2
            | PacketOpcode::ZC_NOTIFY_UNREADMAIL
            | PacketOpcode::ZC_ACH_UPDATE
            | PacketOpcode::ZC_OVERWEIGHT_PERCENT
            | PacketOpcode::ZC_REQ_WEAR_EQUIP_ACK
            | PacketOpcode::ZC_REQ_TAKEOFF_EQUIP_ACK
            | PacketOpcode::ZC_ACK_TAKEOFF_EQUIP_ALL
            | PacketOpcode::ZC_EQUIP_ARROW
            | PacketOpcode::ZC_ACTION_FAILURE
            | PacketOpcode::ZC_MSG
            | PacketOpcode::ZC_MSG_STATE_CHANGE3
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_core::{
        AccountId, CharId, CharServerInfo, CharacterLooks, CharacterSummary, MapPosition, Sex,
        TARGET_PACKETVER,
    };

    #[test]
    fn test_phase0_event_encoder_all_variants() {
        let encoder = Phase0EventEncoder;
        let ver = TARGET_PACKETVER;

        // 1. AcceptEnter -> 23-byte burst (0x0283, 0x0b18, 0x02eb)
        let event = ServerEvent::AcceptEnter {
            account_id: AccountId(2000000),
            server_tick: 5000,
            pos: MapPosition {
                x: 100,
                y: 120,
                dir: 4,
            },
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 23);
        assert_eq!(&bytes[0..2], &PacketOpcode::ZC_AID.raw().to_le_bytes());

        // 1b. Aid -> 0x0283 (6 bytes)
        let event = ServerEvent::Aid {
            account_id: AccountId(2000000),
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &PacketOpcode::ZC_AID.raw().to_le_bytes());

        // 1c. InventoryExpansionInfo -> 0x0b18 (4 bytes)
        let event = ServerEvent::InventoryExpansionInfo { expansion_size: 0 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 4);
        assert_eq!(
            &bytes[0..2],
            &PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
                .raw()
                .to_le_bytes()
        );

        // 2. TimeSynchronized -> 0x007f (6 bytes)
        let event = ServerEvent::TimeSynchronized { server_tick: 9999 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &0x007fu16.to_le_bytes());

        // 6. ParameterChanged -> 0x00b0 (8 bytes)
        let event = ServerEvent::ParameterChanged {
            var_id: 10,
            value: 42,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &0x00b0u16.to_le_bytes());

        // 7. EntityAction supported -> 0x08c8 (34 bytes)
        let event = ServerEvent::EntityAction {
            source_id: 100,
            target_id: 200,
            action_type: 0,
            action_tick: 4321,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 34);
        assert_eq!(&bytes[0..2], &0x08c8u16.to_le_bytes());

        // 8. EntityAction unsupported
        let event = ServerEvent::EntityAction {
            source_id: 100,
            target_id: 200,
            action_type: 99,
            action_tick: 4321,
        };
        let err = encoder.encode_event(&event, ver).unwrap_err();
        match err {
            PacketCodecError::UnsupportedEvent(msg) => {
                assert!(msg.contains("99"));
            }
            _ => panic!("Expected UnsupportedEvent error"),
        }

        // 9. LoginAccepted -> 0x0ac4 (64 + 160 bytes)
        let event = ServerEvent::LoginAccepted {
            account_id: AccountId(2000001),
            login_id1: 1111,
            login_id2: 2222,
            sex: Sex::Male,
            char_servers: vec![CharServerInfo {
                ip: [127, 0, 0, 1],
                port: 6121,
                name: "Loki".to_string(),
                users: 10,
                server_type: 0,
                new_server: 0,
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0ac4u16.to_le_bytes());
        assert_eq!(bytes.len(), 64 + 160);

        // 10. LoginRefused -> 0x083e (26 bytes)
        let event = ServerEvent::LoginRefused { error_code: 1 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x083eu16.to_le_bytes());
        assert_eq!(bytes.len(), 26);
        assert_eq!(bytes[2], 1);

        // 11. CharacterListLoaded
        let event = ServerEvent::CharacterListLoaded {
            account_id: AccountId(2000001),
            total_slots: 9,
            characters: vec![CharacterSummary {
                char_id: CharId(150001),
                account_id: AccountId(2000001),
                slot: 0,
                name: "Hero".to_string(),
                sex: Sex::Male,
                job_class: 0,
                base_level: 1,
                job_level: 1,
                hair_style: 1,
                hair_color: 2,
                last_map: "prontera.gat".to_string(),
                last_pos: MapPosition {
                    x: 50,
                    y: 50,
                    dir: 0,
                },
                delete_date: None,
                looks: CharacterLooks::default(),
                select_stats: Default::default(),
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x082du16.to_le_bytes());
        assert_eq!(&bytes[2..4], &29u16.to_le_bytes());
        assert_eq!(&bytes[29..31], &0x006bu16.to_le_bytes());
        assert_eq!(&bytes[31..33], &202u16.to_le_bytes());
        assert_eq!(&bytes[231..233], &0x09a0u16.to_le_bytes());
        assert_eq!(&bytes[233..237], &3u32.to_le_bytes());
        assert_eq!(&bytes[237..239], &0x020du16.to_le_bytes());
        assert_eq!(&bytes[239..241], &4u16.to_le_bytes());
        assert_eq!(&bytes[241..243], &0x08b9u16.to_le_bytes());
        assert_eq!(&bytes[243..247], &0u32.to_le_bytes());
        assert_eq!(&bytes[247..251], &2000001u32.to_le_bytes());
        assert_eq!(&bytes[251..253], &0u16.to_le_bytes());
        assert_eq!(bytes.len(), 253);

        // 12. CharacterSelected
        let event = ServerEvent::CharacterSelected {
            char_id: CharId(150001),
            map_name: "prontera.gat".to_string(),
            map_host: [127, 0, 0, 1],
            map_port: 5121,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0ac5u16.to_le_bytes());
        assert_eq!(bytes.len(), 156);
        assert_eq!(&bytes[2..6], &150001u32.to_le_bytes());

        // 13. CharacterPageLoaded
        let event = ServerEvent::CharacterPageLoaded {
            characters: vec![CharacterSummary {
                char_id: CharId(150002),
                account_id: AccountId(2000001),
                slot: 1,
                name: "Mage".to_string(),
                sex: Sex::Female,
                job_class: 1,
                base_level: 10,
                job_level: 5,
                hair_style: 2,
                hair_color: 3,
                last_map: "geffen.gat".to_string(),
                last_pos: MapPosition {
                    x: 100,
                    y: 100,
                    dir: 2,
                },
                delete_date: None,
                looks: CharacterLooks::default(),
                select_stats: Default::default(),
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0b72u16.to_le_bytes());
        assert_eq!(bytes.len(), 4 + 175);

        // 14. CharacterCreated
        let event = ServerEvent::CharacterCreated {
            character: CharacterSummary {
                char_id: CharId(150003),
                account_id: AccountId(2000001),
                slot: 2,
                name: "Thief".to_string(),
                sex: Sex::Male,
                job_class: 6,
                base_level: 1,
                job_level: 1,
                hair_style: 4,
                hair_color: 1,
                last_map: "morocc.gat".to_string(),
                last_pos: MapPosition {
                    x: 50,
                    y: 50,
                    dir: 0,
                },
                delete_date: None,
                looks: CharacterLooks::default(),
                select_stats: Default::default(),
            },
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0b6fu16.to_le_bytes());
        assert_eq!(bytes.len(), 177);

        // 15. CharacterCreateRefused
        let event = ServerEvent::CharacterCreateRefused { error_code: 0x00 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.as_ref(), &[0x6e, 0x00, 0x00]);

        // 16. CharacterDeleteReserved
        let event = ServerEvent::CharacterDeleteReserved {
            char_id: CharId(150004),
            result: 1,
            delete_delay_seconds: 86400,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        let expected_del_res: [u8; 14] = [
            0x28, 0x08, 0xf4, 0x49, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x51, 0x01, 0x00,
        ];
        assert_eq!(bytes.as_ref(), &expected_del_res);

        // 17. CharacterDeleteCancelled
        let event = ServerEvent::CharacterDeleteCancelled {
            char_id: CharId(150004),
            result: 1,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x082cu16.to_le_bytes());
        assert_eq!(&bytes[2..6], &150004u32.to_le_bytes());
        assert_eq!(&bytes[6..10], &1u32.to_le_bytes());

        // 18. RestartAck
        let event = ServerEvent::RestartAck { result: 1 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.as_ref(), &[0xb3, 0x00, 0x01]);

        // 19. DisconnectAck (PKT-QUIT-02)
        let event = ServerEvent::DisconnectAck { result: 0 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.as_ref(), &[0x8b, 0x01, 0x00, 0x00]);
        assert!(is_implemented_outbound_opcode(
            PacketOpcode::ZC_ACK_REQ_DISCONNECT,
            ver
        ));
    }

    #[test]
    fn test_modern_event_encoder_20260219() {
        let encoder = ModernEventEncoder;
        let ver = 20260219;

        let event = ServerEvent::AcceptEnter {
            account_id: AccountId(2000000),
            server_tick: 5000,
            pos: MapPosition {
                x: 100,
                y: 120,
                dir: 4,
            },
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 23);
        assert_eq!(&bytes[0..2], &PacketOpcode::ZC_AID.raw().to_le_bytes());

        let event = ServerEvent::Aid {
            account_id: AccountId(2000000),
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &PacketOpcode::ZC_AID.raw().to_le_bytes());

        let event = ServerEvent::InventoryExpansionInfo { expansion_size: 0 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 4);
        assert_eq!(
            &bytes[0..2],
            &PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
                .raw()
                .to_le_bytes()
        );

        let event = ServerEvent::TimeSynchronized { server_tick: 9999 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 6);
        assert_eq!(&bytes[0..2], &0x007fu16.to_le_bytes());

        let event = ServerEvent::ParameterChanged {
            var_id: 10,
            value: 42,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &0x00b0u16.to_le_bytes());

        let event = ServerEvent::EntityAction {
            source_id: 100,
            target_id: 200,
            action_type: 0,
            action_tick: 4321,
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(bytes.len(), 34);
        assert_eq!(&bytes[0..2], &0x08c8u16.to_le_bytes());

        let event = ServerEvent::LoginAccepted {
            account_id: AccountId(2000001),
            login_id1: 1111,
            login_id2: 2222,
            sex: Sex::Male,
            char_servers: vec![CharServerInfo {
                ip: [127, 0, 0, 1],
                port: 6121,
                name: "Loki".to_string(),
                users: 10,
                server_type: 0,
                new_server: 0,
            }],
        };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x0ac4u16.to_le_bytes());
        assert_eq!(bytes.len(), 64 + 160);

        let event = ServerEvent::LoginRefused { error_code: 1 };
        let bytes = encoder.encode_event(&event, ver).unwrap();
        assert_eq!(&bytes[0..2], &0x083eu16.to_le_bytes());
        assert_eq!(bytes.len(), 26);
    }

    #[test]
    fn test_is_implemented_outbound_opcode() {
        let ver = 20260219;
        let implemented = [
            PacketOpcode::HC_ACCEPT_ENTER,
            PacketOpcode::HC_REFUSE_ENTER,
            PacketOpcode::ZC_NOTIFY_TIME,
            PacketOpcode::ZC_NOTIFY_VANISH,
            PacketOpcode::ZC_NOTIFY_PLAYERMOVE,
            PacketOpcode::ZC_PAR_CHANGE,
            PacketOpcode::ZC_ACCEPT_ENTER,
            PacketOpcode::HC_ACCEPT_ENTER2,
            PacketOpcode::AC_REFUSE_LOGIN,
            PacketOpcode::ZC_NOTIFY_ACT,
            PacketOpcode::HC_ACK_CHARINFO_PER_PAGE,
            PacketOpcode::HC_ACCEPT_MAKECHAR,
            PacketOpcode::HC_REFUSE_MAKECHAR,
            PacketOpcode::HC_DELETE_CHAR3_RESERVED,
            PacketOpcode::HC_DELETE_CHAR3_CANCEL,
            PacketOpcode::ZC_RESTART_ACK,
            PacketOpcode::HC_CHARLIST_NOTIFY,
            PacketOpcode::HC_BLOCK_CHARACTER,
            PacketOpcode::HC_SECOND_PASSWD_LOGIN,
            PacketOpcode::ZC_NOTIFY_MOVEENTRY11,
            PacketOpcode::ZC_NOTIFY_NEWENTRY11,
            PacketOpcode::ZC_NOTIFY_STANDENTRY11,
            PacketOpcode::AC_ACCEPT_LOGIN,
            PacketOpcode::HC_NOTIFY_ZONESVR,
            PacketOpcode::ZC_AID,
            PacketOpcode::ZC_INVENTORY_EXPANSION_INFO,
        ];

        for op in implemented {
            assert!(
                is_implemented_outbound_opcode(op, ver),
                "Opcode {:#06x} should be marked as implemented",
                op.raw()
            );
        }

        let unimplemented = [
            PacketOpcode::CA_LOGIN,
            PacketOpcode::CZ_ENTER,
            PacketOpcode::CZ_REQUEST_MOVE,
            PacketOpcode::ZC_ACK_REQNAME_BYGID,
            PacketOpcode(0x1234),
        ];

        for op in unimplemented {
            assert!(
                !is_implemented_outbound_opcode(op, ver),
                "Opcode {:#06x} should not be marked as implemented",
                op.raw()
            );
        }
    }
}
