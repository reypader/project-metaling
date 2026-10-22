//! Event encoder connecting domain ServerEvent variants to outgoing packet encoders.

use bytes::{BufMut, Bytes, BytesMut};
use ro_contracts::packet::{EventEncoder, PacketCodecError, PacketOpcode, ServerEvent};

use super::char::{
    encode_hc_accept_enter, encode_hc_accept_enter2, encode_hc_accept_makechar,
    encode_hc_ack_charinfo_per_page, encode_hc_delete_char3_cancel,
    encode_hc_delete_char3_reserved, encode_hc_notify_zonesvr, encode_hc_refuse_enter,
    encode_hc_refuse_makechar,
};
use super::login::{encode_ac_accept_login, encode_ac_refuse_login, CharServerEntry};
use super::map::{
    encode_zc_accept_enter, encode_zc_aid, encode_zc_inventory_expansion_info,
    encode_zc_notify_act, encode_zc_notify_newentry, encode_zc_notify_time,
    encode_zc_par_change_i32, encode_zc_restart_ack, NewEntryParams,
};

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
            // Transitional (M1): real encoders for every new unit event land in M3.
            ServerEvent::UnitSpawned { unit } => {
                let params = NewEntryParams {
                    gid: unit.gid,
                    speed: unit.speed as i16,
                    body_state: 0,
                    health_state: 0,
                    effect_state: 0,
                    job: unit.job as i16,
                    head: unit.hair_style as i16,
                    weapon: 0,
                    accessory: 0,
                    shield: 0,
                    accessory2: 0,
                    accessory3: 0,
                    headpalette: unit.hair_color as i16,
                    bodypalette: 0,
                    head_dir: unit.head_dir as i16,
                    guid: 0,
                    gemblem_ver: 0,
                    honor: 0,
                    virtue: 0,
                    is_pkmode_on: 0,
                    sex: unit.sex,
                    pos: unit.pos,
                    x_size: 5,
                    y_size: 5,
                    clevel: unit.clevel as i16,
                };
                Ok(encode_zc_notify_newentry(&params))
            }
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
                } else {
                    Err(PacketCodecError::UnsupportedEvent(format!(
                        "Action type {action_type} is not implemented"
                    )))
                }
            }
            ServerEvent::RestartAck { result } => Ok(encode_zc_restart_ack(*result)),
            #[allow(unreachable_patterns)]
            unsupported => Err(PacketCodecError::UnsupportedEvent(format!(
                "{unsupported:?}"
            ))),
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
            | PacketOpcode::HC_CHARLIST_NOTIFY
            | PacketOpcode::HC_BLOCK_CHARACTER
            | PacketOpcode::HC_SECOND_PASSWD_LOGIN
            | PacketOpcode::ZC_NOTIFY_MOVEENTRY11
            | PacketOpcode::ZC_NOTIFY_NEWENTRY11
            | PacketOpcode::ZC_NOTIFY_STANDENTRY11
            | PacketOpcode::AC_ACCEPT_LOGIN
            | PacketOpcode::HC_NOTIFY_ZONESVR
            | PacketOpcode::ZC_AID
            | PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_contracts::ids::{
        AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, Sex,
    };
    use ro_contracts::TARGET_PACKETVER;

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
            PacketOpcode(0x0091),
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
