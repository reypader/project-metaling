//! Integration tests for packet framing, coordinate packing, and packet codecs.

use bytes::BytesMut;
use ro_core::{
    AccountId, CharId, CharServerInfo, CharacterLooks, CharacterSummary, ClientIntent, MapPosition,
    ServerEvent, Sex, UnitAppearance, UnitLooks, UnitObjectType,
};
use ro_packets::{
    decode_frame, encode_ac_accept_login, encode_ac_refuse_login, encode_hc_notify_zonesvr,
    encode_zc_accept_enter, encode_zc_notify_act, encode_zc_notify_act_params,
    encode_zc_notify_playermove, encode_zc_notify_time, encode_zc_par_change, pack_pos2,
    pack_pos_dir, unpack_pos2, unpack_pos_dir, ActParams, CharServerEntry, DecodedFrame,
    EventEncoder, ModernEventEncoder, PacketDecoder, PacketOpcode, Phase0EventEncoder,
    RathenaPacketLengthTable,
};
use tokio_util::codec::Decoder;

#[test]
fn test_stream_fragmentation_and_coalescing() {
    let mut decoder = PacketDecoder::default();
    let mut stream_buffer = BytesMut::new();

    // Prepare 3 packets:
    // 1. CZ_ENTER (0x0436, 23 bytes)
    let mut cz_enter_bytes = vec![0x36, 0x04];
    cz_enter_bytes.extend_from_slice(&2000001u32.to_le_bytes()); // account_id
    cz_enter_bytes.extend_from_slice(&150001u32.to_le_bytes()); // char_id
    cz_enter_bytes.extend_from_slice(&12345u32.to_le_bytes()); // login_id1
    cz_enter_bytes.extend_from_slice(&99999u32.to_le_bytes()); // client_tick
    cz_enter_bytes.extend_from_slice(&[0u8; 4]); // padding
    cz_enter_bytes.push(1); // Sex::Male

    // 2. CZ_REQUEST_TIME (0x0360, 6 bytes)
    let mut cz_time_bytes = vec![0x60, 0x03];
    cz_time_bytes.extend_from_slice(&100001u32.to_le_bytes());

    // 3. CZ_NOTIFY_ACTORINIT (0x007d, 2 bytes)
    let cz_actorinit_bytes = vec![0x7d, 0x00];

    // Combine all 3 packets into one byte stream
    let mut all_bytes = Vec::new();
    all_bytes.extend_from_slice(&cz_enter_bytes);
    all_bytes.extend_from_slice(&cz_time_bytes);
    all_bytes.extend_from_slice(&cz_actorinit_bytes);

    // Feed bytes into the decoder 1 byte at a time to test fragmentation resilience
    let mut decoded_packets = Vec::new();

    for byte in all_bytes {
        stream_buffer.extend_from_slice(&[byte]);
        while let Some(packet) = decoder.decode(&mut stream_buffer).unwrap() {
            decoded_packets.push(packet);
        }
    }

    assert_eq!(decoded_packets.len(), 3);

    // Check packet 1
    assert_eq!(decoded_packets[0].opcode, PacketOpcode(0x0436));
    let frame1 = decode_frame(&decoded_packets[0], ro_core::TARGET_PACKETVER).unwrap();
    assert_eq!(
        frame1,
        DecodedFrame::Intent(ClientIntent::EnterMap {
            account_id: AccountId(2000001),
            char_id: CharId(150001),
            login_id1: 12345,
            client_tick: 99999,
            sex: Sex::Male,
        })
    );

    // Check packet 2
    assert_eq!(decoded_packets[1].opcode, PacketOpcode(0x0360));
    let frame2 = decode_frame(&decoded_packets[1], ro_core::TARGET_PACKETVER).unwrap();
    assert_eq!(
        frame2,
        DecodedFrame::Intent(ClientIntent::RequestTime {
            client_tick: 100001,
        })
    );

    // Check packet 3
    assert_eq!(decoded_packets[2].opcode, PacketOpcode(0x007d));
    let frame3 = decode_frame(&decoded_packets[2], ro_core::TARGET_PACKETVER).unwrap();
    assert_eq!(frame3, DecodedFrame::Intent(ClientIntent::NotifyActorInit));

    assert!(stream_buffer.is_empty());
}

#[test]
fn test_unhandled_packet_consumption() {
    let mut decoder = PacketDecoder::default();
    let mut stream = BytesMut::new();

    // 0x00bf is CZ_REQ_EMOTION (3 bytes fixed in clif_packetdb.hpp)
    stream.extend_from_slice(&[0xbf, 0x00, 0x07]);

    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("packet expected");
    assert_eq!(raw.opcode, PacketOpcode(0x00bf));
    assert_eq!(raw.payload.as_ref(), &[0x07]);
    assert!(stream.is_empty());

    let frame = decode_frame(&raw, ro_core::TARGET_PACKETVER).unwrap();
    match frame {
        DecodedFrame::Unhandled { opcode, len } => {
            assert_eq!(opcode, PacketOpcode(0x00bf));
            assert_eq!(len, 3);
        }
        _ => panic!("Expected unhandled frame"),
    }
}

#[test]
fn test_outgoing_serializers_and_redecoding() {
    let mut decoder = PacketDecoder::default();

    // 1. AC_ACCEPT_LOGIN (dynamic)
    let servers = [CharServerEntry {
        ip: [192, 168, 1, 100],
        port: 6121,
        name: "Thor".to_string(),
        users: 15,
        server_type: 0,
        new_server: 0,
    }];
    let login_bytes = encode_ac_accept_login(
        1001,
        AccountId(2000001),
        1002,
        [192, 168, 1, 50],
        "2026-10-16 08:00:00",
        Sex::Female,
        &servers,
    )
    .unwrap();
    let mut stream = BytesMut::from(login_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("AC_ACCEPT_LOGIN decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0ac4));
    assert_eq!(raw.payload_len(), 64 + 160 - 4);

    // 2. AC_REFUSE_LOGIN (26 bytes)
    let refuse_bytes = encode_ac_refuse_login(3, "0");
    let mut stream = BytesMut::from(refuse_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("AC_REFUSE_LOGIN decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x083e));
    assert_eq!(raw.payload_len(), 24);

    // 3. HC_NOTIFY_ZONESVR (156 bytes)
    let zone_bytes = encode_hc_notify_zonesvr(CharId(150001), "geffen.gat", [127, 0, 0, 1], 5121);
    let mut stream = BytesMut::from(zone_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("HC_NOTIFY_ZONESVR decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0ac5));
    assert_eq!(raw.payload_len(), 154);

    // 4. ZC_ACCEPT_ENTER (13 bytes)
    let accept_bytes = encode_zc_accept_enter(
        12345,
        MapPosition {
            x: 50,
            y: 75,
            dir: 2,
        },
        5,
        5,
    );
    let mut stream = BytesMut::from(accept_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_ACCEPT_ENTER decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x02eb));
    assert_eq!(raw.payload_len(), 11);

    // 5. ZC_NOTIFY_TIME (6 bytes)
    let time_bytes = encode_zc_notify_time(8888);
    let mut stream = BytesMut::from(time_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_NOTIFY_TIME decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x007f));
    assert_eq!(raw.payload_len(), 4);

    // 6. ZC_NOTIFY_NEWENTRY11 (107 bytes)
    let newentry_bytes = ModernEventEncoder
        .encode_event(
            &ServerEvent::UnitSpawned {
                unit: UnitAppearance {
                    object_type: UnitObjectType::Pc,
                    gid: 2000001,
                    char_id: Some(CharId(150001)),
                    name: "Novice".into(),
                    job: 1,
                    sex: Sex::Female,
                    hair_style: 2,
                    hair_color: 0,
                    head_dir: 0,
                    pos: MapPosition {
                        x: 100,
                        y: 150,
                        dir: 6,
                    },
                    speed: 150,
                    dead_sit: 0,
                    clevel: 10,
                    effect_state: 0,
                    looks: UnitLooks::default(),
                },
            },
            ro_core::TARGET_PACKETVER,
        )
        .expect("UnitSpawned encodes");
    let mut stream = BytesMut::from(newentry_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_NOTIFY_NEWENTRY decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x09fe));
    assert_eq!(raw.payload_len(), 103);

    // 7. ZC_PAR_CHANGE (8 bytes)
    let par_bytes = encode_zc_par_change(5, 99);
    let mut stream = BytesMut::from(par_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_PAR_CHANGE decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x00b0));
    assert_eq!(raw.payload_len(), 6);

    // 8. ZC_NOTIFY_PLAYERMOVE (12 bytes)
    let move_bytes = encode_zc_notify_playermove(10, 20, 30, 40, 5000);
    let mut stream = BytesMut::from(move_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_NOTIFY_PLAYERMOVE decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0087));
    assert_eq!(raw.payload_len(), 10);

    // 9. ZC_NOTIFY_ACT (34 bytes)
    let act_bytes = encode_zc_notify_act(100, 200, 1000, 0, 0, 10, 1, 0, 0);
    let mut stream = BytesMut::from(act_bytes.as_ref());
    let raw = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_NOTIFY_ACT decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x08c8));
    assert_eq!(raw.payload_len(), 32);

    // 10. ZC_NOTIFY_ACT via ActParams (34 bytes)
    let act_params = ActParams {
        source_id: 100,
        target_id: 200,
        server_tick: 1000,
        attack_speed: 0,
        damage_delay: 0,
        damage: 10,
        div_count: 1,
        action_type: 0,
        left_damage: 0,
    };
    let act_params_bytes = encode_zc_notify_act_params(&act_params);
    assert_eq!(act_bytes, act_params_bytes);
}

#[test]
fn test_event_encoder_integration() {
    let encoder = Phase0EventEncoder;
    let ver = ro_core::TARGET_PACKETVER;
    let mut decoder = PacketDecoder::default();

    // 1. AcceptEnter (23-byte burst: 0x0283, 0x0b18, 0x02eb)
    let event = ServerEvent::AcceptEnter {
        account_id: AccountId(2000001),
        server_tick: 1111,
        pos: MapPosition {
            x: 50,
            y: 60,
            dir: 1,
        },
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    assert_eq!(bytes.len(), 23);
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw1 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw1.opcode, PacketOpcode::ZC_AID);
    let raw2 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw2.opcode, PacketOpcode::ZC_INVENTORY_EXPANSION_INFO);
    let raw3 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw3.opcode, PacketOpcode::ZC_ACCEPT_ENTER);

    // 2. LoginAccepted (0x0ac4)
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
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0ac4));

    // 3. LoginRefused (0x083e)
    let event = ServerEvent::LoginRefused { error_code: 6 };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x083e));

    // 4. CharacterListLoaded (produces 5-packet burst: 0x082d, 0x006b, 0x09a0, 0x020d, 0x08b9)
    let event = ServerEvent::CharacterListLoaded {
        account_id: AccountId(2000001),
        total_slots: 9,
        characters: vec![CharacterSummary {
            char_id: CharId(150001),
            account_id: AccountId(2000001),
            slot: 0,
            name: "Valkyrie".to_string(),
            sex: Sex::Female,
            job_class: 4001,
            base_level: 99,
            job_level: 70,
            hair_style: 3,
            hair_color: 5,
            last_map: "morocc.gat".to_string(),
            last_pos: MapPosition {
                x: 150,
                y: 150,
                dir: 4,
            },
            delete_date: None,
            looks: CharacterLooks::default(),
        }],
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw1 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw1.opcode, PacketOpcode(0x082d));
    let raw2 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw2.opcode, PacketOpcode(0x006b));
    let raw3 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw3.opcode, PacketOpcode(0x09a0));
    let raw4 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw4.opcode, PacketOpcode(0x020d));
    let raw5 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw5.opcode, PacketOpcode(0x08b9));
    assert!(stream.is_empty());

    // 5. CharacterPageLoaded (produces HC_ACK_CHARINFO_PER_PAGE: 0x0b72)
    let page_event = ServerEvent::CharacterPageLoaded {
        characters: vec![CharacterSummary {
            char_id: CharId(150001),
            account_id: AccountId(2000001),
            slot: 0,
            name: "Valkyrie".to_string(),
            sex: Sex::Female,
            job_class: 4001,
            base_level: 99,
            job_level: 70,
            hair_style: 3,
            hair_color: 5,
            last_map: "morocc.gat".to_string(),
            last_pos: MapPosition {
                x: 150,
                y: 150,
                dir: 4,
            },
            delete_date: None,
            looks: CharacterLooks::default(),
        }],
    };
    let page_bytes = encoder.encode_event(&page_event, ver).unwrap();
    let mut page_stream = BytesMut::from(page_bytes.as_ref());
    let raw_page = decoder.decode(&mut page_stream).unwrap().unwrap();
    assert_eq!(raw_page.opcode, PacketOpcode(0x0b72));
    assert!(page_stream.is_empty());

    // 6. CharacterSelected
    let event = ServerEvent::CharacterSelected {
        char_id: CharId(150001),
        map_name: "prontera.gat".to_string(),
        map_host: [192, 168, 1, 10],
        map_port: 5121,
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0ac5));
}

#[test]
fn test_login_and_char_intent_decoding() {
    let ver = ro_core::TARGET_PACKETVER;

    // 1. CA_LOGIN
    let mut ca_login_raw = vec![0x64, 0x00];
    ca_login_raw.extend_from_slice(&ver.to_le_bytes());
    let mut username = [0u8; 24];
    username[..4].copy_from_slice(b"user");
    ca_login_raw.extend_from_slice(&username);
    let mut password = [0u8; 24];
    password[..4].copy_from_slice(b"pass");
    ca_login_raw.extend_from_slice(&password);
    ca_login_raw.push(1); // client_type

    let mut decoder = PacketDecoder::default();
    let mut stream = BytesMut::from(ca_login_raw.as_slice());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0064));
    let frame = decode_frame(&raw, ver).unwrap();
    assert_eq!(
        frame,
        DecodedFrame::Intent(ClientIntent::Login {
            username: "user".to_string(),
            password: "pass".to_string(),
            client_version: ver,
            client_type: 1,
        })
    );

    // 2. CH_ENTER
    let mut ch_enter_raw = vec![0x65, 0x00];
    ch_enter_raw.extend_from_slice(&2000001u32.to_le_bytes());
    ch_enter_raw.extend_from_slice(&11111u32.to_le_bytes());
    ch_enter_raw.extend_from_slice(&22222u32.to_le_bytes());
    ch_enter_raw.extend_from_slice(&30u16.to_le_bytes());
    ch_enter_raw.push(1); // Male

    let mut stream = BytesMut::from(ch_enter_raw.as_slice());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0065));
    let frame = decode_frame(&raw, ver).unwrap();
    assert_eq!(
        frame,
        DecodedFrame::Intent(ClientIntent::EnterCharServer {
            account_id: AccountId(2000001),
            login_id1: 11111,
            login_id2: 22222,
            client_version: 30,
            sex: Sex::Male,
        })
    );

    // 3. CH_SELECT_CHAR
    let ch_select_raw = vec![0x66, 0x00, 0x04];
    let mut stream = BytesMut::from(ch_select_raw.as_slice());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0066));
    let frame = decode_frame(&raw, ver).unwrap();
    assert_eq!(
        frame,
        DecodedFrame::Intent(ClientIntent::SelectCharacter { slot: 4 })
    );
}

#[test]
fn test_all_directions_coords_packing() {
    for dir in 0..16 {
        for &(x, y) in &[(0, 0), (1, 1), (500, 500), (1023, 1023)] {
            let packed = pack_pos_dir(x, y, dir);
            let (ux, uy, udir) = unpack_pos_dir(&packed);
            assert_eq!(x, ux);
            assert_eq!(y, uy);
            assert_eq!(dir, udir);
        }
    }
}

#[test]
fn test_pos2_roundtrip_exhaustive() {
    let test_points = [
        (0, 0, 0, 0, 0, 0),
        (100, 200, 150, 250, 8, 8),
        (1023, 1023, 1023, 1023, 15, 15),
        (512, 256, 128, 64, 4, 12),
    ];

    for (x0, y0, x1, y1, sx0, sy0) in test_points {
        let packed = pack_pos2(x0, y0, x1, y1, sx0, sy0);
        let (ux0, uy0, ux1, uy1, usx0, usy0) = unpack_pos2(&packed);
        assert_eq!(x0, ux0);
        assert_eq!(y0, uy0);
        assert_eq!(x1, ux1);
        assert_eq!(y1, uy1);
        assert_eq!(sx0, usx0);
        assert_eq!(sy0, usy0);
    }
}

#[test]
fn test_modern_framing_and_intent_parsing() {
    let ver = 20260219;
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, ver);
    let mut stream_buffer = BytesMut::new();

    // Prepare modern stream:
    // 1. CZ_ENTER (0x0436, 23 bytes)
    let mut cz_enter = vec![0x36, 0x04];
    cz_enter.extend_from_slice(&2000001u32.to_le_bytes());
    cz_enter.extend_from_slice(&150001u32.to_le_bytes());
    cz_enter.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter.extend_from_slice(&99999u32.to_le_bytes());
    cz_enter.extend_from_slice(&[0u8; 4]);
    cz_enter.push(0); // Female

    // 2. CZ_REQUEST_TIME (0x0360, 6 bytes)
    let mut cz_time = vec![0x60, 0x03];
    cz_time.extend_from_slice(&88888u32.to_le_bytes());

    // 3. CZ_REQUEST_MOVE (0x035f, 5 bytes)
    let mut cz_move = vec![0x5f, 0x03];
    cz_move.extend_from_slice(&pack_pos_dir(120, 240, 0));

    // 4. CZ_CHANGE_DIRECTION (0x0361, 5 bytes)
    let mut cz_dir = vec![0x61, 0x03];
    cz_dir.extend_from_slice(&2u16.to_le_bytes());
    cz_dir.push(4);

    // 5. CZ_REQUEST_ACT (0x0437, 7 bytes)
    let mut cz_act = vec![0x37, 0x04];
    cz_act.extend_from_slice(&50001u32.to_le_bytes());
    cz_act.push(2);

    // 6. CZ_REQNAME (0x0368, 6 bytes)
    let mut cz_name = vec![0x68, 0x03];
    cz_name.extend_from_slice(&60001u32.to_le_bytes());

    let mut all_bytes = Vec::new();
    all_bytes.extend_from_slice(&cz_enter);
    all_bytes.extend_from_slice(&cz_time);
    all_bytes.extend_from_slice(&cz_move);
    all_bytes.extend_from_slice(&cz_dir);
    all_bytes.extend_from_slice(&cz_act);
    all_bytes.extend_from_slice(&cz_name);

    let mut decoded = Vec::new();
    for chunk in all_bytes.chunks(3) {
        stream_buffer.extend_from_slice(chunk);
        while let Some(packet) = decoder.decode(&mut stream_buffer).unwrap() {
            decoded.push(packet);
        }
    }

    assert_eq!(decoded.len(), 6);
    assert_eq!(decoded[0].opcode, PacketOpcode(0x0436));
    assert_eq!(
        decode_frame(&decoded[0], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::EnterMap {
            account_id: AccountId(2000001),
            char_id: CharId(150001),
            login_id1: 12345,
            client_tick: 99999,
            sex: Sex::Female,
        })
    );

    assert_eq!(decoded[1].opcode, PacketOpcode(0x0360));
    assert_eq!(
        decode_frame(&decoded[1], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::RequestTime { client_tick: 88888 })
    );

    assert_eq!(decoded[2].opcode, PacketOpcode(0x035f));
    assert_eq!(
        decode_frame(&decoded[2], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::RequestMove {
            dest_x: 120,
            dest_y: 240
        })
    );

    assert_eq!(decoded[3].opcode, PacketOpcode(0x0361));
    assert_eq!(
        decode_frame(&decoded[3], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4
        })
    );

    assert_eq!(decoded[4].opcode, PacketOpcode(0x0437));
    assert_eq!(
        decode_frame(&decoded[4], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::RequestAction {
            target_id: 50001,
            action_type: 2
        })
    );

    assert_eq!(decoded[5].opcode, PacketOpcode(0x0368));
    assert_eq!(
        decode_frame(&decoded[5], ver).unwrap(),
        DecodedFrame::Intent(ClientIntent::RequestEntityName { entity_id: 60001 })
    );
}

#[test]
fn test_modern_event_encoder_roundtrip() {
    let encoder = ModernEventEncoder;
    let ver = 20260219;
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, ver);

    // 1. ZC_ACCEPT_ENTER_MODERN burst (23 bytes: 0x0283, 0x0b18, 0x02eb)
    let event = ServerEvent::AcceptEnter {
        account_id: AccountId(2000001),
        server_tick: 7777,
        pos: MapPosition {
            x: 55,
            y: 66,
            dir: 3,
        },
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    assert_eq!(bytes.len(), 23);
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw1 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw1.opcode, PacketOpcode::ZC_AID);
    assert_eq!(raw1.payload_len(), 4);
    let raw2 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw2.opcode, PacketOpcode::ZC_INVENTORY_EXPANSION_INFO);
    assert_eq!(raw2.payload_len(), 2);
    let raw3 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw3.opcode, PacketOpcode(0x02eb));
    assert_eq!(raw3.payload_len(), 11);

    // 3. ZC_NOTIFY_ACT_MODERN (0x08c8, 34 bytes)
    let event = ServerEvent::EntityAction {
        source_id: 100,
        target_id: 200,
        action_type: 2,
        action_tick: 1234,
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x08c8));
    assert_eq!(raw.payload_len(), 32);
}

#[test]
fn test_login_sequence_trace_exact_byte_layouts() {
    let ver = ro_core::TARGET_PACKETVER;
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, ver);
    let encoder = ModernEventEncoder;

    // 1. Initial CA_LOGIN with incorrect password "passwords"
    let mut ca_login_fail = vec![0x64, 0x00];
    ca_login_fail.extend_from_slice(&0x80000000u32.to_le_bytes());
    let mut username = [0u8; 24];
    username[..4].copy_from_slice(b"test");
    ca_login_fail.extend_from_slice(&username);
    let mut password_fail = [0u8; 24];
    password_fail[..9].copy_from_slice(b"passwords");
    ca_login_fail.extend_from_slice(&password_fail);
    ca_login_fail.push(0);

    assert_eq!(ca_login_fail.len(), 55);

    let mut stream_fail = BytesMut::from(ca_login_fail.as_slice());
    let raw_fail = decoder
        .decode(&mut stream_fail)
        .unwrap()
        .expect("RawPacket expected");
    assert_eq!(raw_fail.opcode, PacketOpcode(0x0064));
    assert_eq!(raw_fail.payload_len(), 53);

    let frame_fail = decode_frame(&raw_fail, ver).unwrap();
    assert_eq!(
        frame_fail,
        DecodedFrame::Intent(ClientIntent::Login {
            username: "test".to_string(),
            password: "passwords".to_string(),
            client_version: 0x80000000,
            client_type: 0,
        })
    );

    // 2. Server responds with LoginRefused { error_code: 1 }
    let refuse_event = ServerEvent::LoginRefused { error_code: 1 };
    let refuse_bytes = encoder.encode_event(&refuse_event, ver).unwrap();
    assert_eq!(refuse_bytes.len(), 26);

    let expected_refuse_wire: [u8; 26] = [
        0x3e, 0x08, 0x01, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    assert_eq!(refuse_bytes.as_ref(), &expected_refuse_wire);

    let mut stream_refuse = BytesMut::from(refuse_bytes.as_ref());
    let raw_refuse = decoder
        .decode(&mut stream_refuse)
        .unwrap()
        .expect("AC_REFUSE_LOGIN expected");
    assert_eq!(raw_refuse.opcode, PacketOpcode(0x083e));
    assert_eq!(raw_refuse.payload_len(), 24);

    // 3. Retry CA_LOGIN with valid password "password"
    let mut ca_login_retry = vec![0x64, 0x00];
    ca_login_retry.extend_from_slice(&0x80000000u32.to_le_bytes());
    ca_login_retry.extend_from_slice(&username);
    let mut password_success = [0u8; 24];
    password_success[..8].copy_from_slice(b"password");
    ca_login_retry.extend_from_slice(&password_success);
    ca_login_retry.push(0);

    assert_eq!(ca_login_retry.len(), 55);

    let mut stream_retry = BytesMut::from(ca_login_retry.as_slice());
    let raw_retry = decoder
        .decode(&mut stream_retry)
        .unwrap()
        .expect("RawPacket expected");
    assert_eq!(raw_retry.opcode, PacketOpcode(0x0064));
    assert_eq!(raw_retry.payload_len(), 53);

    let frame_retry = decode_frame(&raw_retry, ver).unwrap();
    assert_eq!(
        frame_retry,
        DecodedFrame::Intent(ClientIntent::Login {
            username: "test".to_string(),
            password: "password".to_string(),
            client_version: 0x80000000,
            client_type: 0,
        })
    );

    // 4. Server responds with LoginAccepted
    let accept_event = ServerEvent::LoginAccepted {
        account_id: AccountId(2000000),
        login_id1: 2422544335,
        login_id2: 800520755,
        sex: Sex::Male,
        char_servers: vec![CharServerInfo {
            ip: [127, 0, 0, 1],
            port: 6121,
            name: "Loki".to_string(),
            users: 0,
            server_type: 0,
            new_server: 0,
        }],
    };
    let accept_bytes = encoder.encode_event(&accept_event, ver).unwrap();

    assert_eq!(accept_bytes.len(), 224);
    assert_eq!(&accept_bytes[0..2], &[0xc4, 0x0a]);
    assert_eq!(&accept_bytes[2..4], &224u16.to_le_bytes());
    assert_eq!(&accept_bytes[4..8], &2422544335u32.to_le_bytes());
    assert_eq!(&accept_bytes[8..12], &2000000u32.to_le_bytes());
    assert_eq!(&accept_bytes[12..16], &800520755u32.to_le_bytes());
    assert_eq!(&accept_bytes[16..20], &[0, 0, 0, 0]);
    assert_eq!(&accept_bytes[20..46], &[0u8; 26]);
    assert_eq!(accept_bytes[46], 1);
    assert_eq!(&accept_bytes[47..64], &[0u8; 17]);

    assert_eq!(&accept_bytes[64..68], &[127, 0, 0, 1]);
    assert_eq!(&accept_bytes[68..70], &6121u16.to_le_bytes());
    assert_eq!(&accept_bytes[70..74], b"Loki");
    assert_eq!(&accept_bytes[74..90], &[0u8; 16]);
    assert_eq!(&accept_bytes[90..92], &0u16.to_le_bytes());
    assert_eq!(&accept_bytes[92..94], &0u16.to_le_bytes());
    assert_eq!(&accept_bytes[94..96], &0u16.to_le_bytes());
    assert_eq!(&accept_bytes[96..224], &[0u8; 128]);

    let mut stream_accept = BytesMut::from(accept_bytes.as_ref());
    let raw_accept = decoder
        .decode(&mut stream_accept)
        .unwrap()
        .expect("AC_ACCEPT_LOGIN expected");
    assert_eq!(raw_accept.opcode, PacketOpcode(0x0ac4));
    assert_eq!(raw_accept.payload_len(), 220);
}

/// PKT-02: streaming framing. Partial frames wait, concatenated frames decode in order, and a
/// dynamic length below the 4-byte header is rejected. (A dynamic length can't exceed
/// `MAX_PACKET_SIZE`: the field is a u16 and the cap is 65535.)
#[test]
fn pkt_02_streaming_framing() {
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, ro_core::TARGET_PACKETVER);

    // Partial fixed frame (CZ_REQUEST_TIME, 6 bytes): bytes stay buffered.
    let mut stream = BytesMut::from(&[0x60u8, 0x03, 1, 2][..]);
    assert!(decoder.decode(&mut stream).expect("partial ok").is_none());
    assert_eq!(stream.len(), 4, "partial bytes are kept");
    stream.extend_from_slice(&[3, 4]);
    let raw = decoder.decode(&mut stream).expect("ok").expect("complete");
    assert_eq!(raw.opcode, PacketOpcode(0x0360));
    assert_eq!(raw.payload.as_ref(), &[1, 2, 3, 4]);

    // Partial dynamic header, then the rest.
    let mut stream = BytesMut::from(&[0xff_u8, 0x09, 6][..]);
    assert!(decoder.decode(&mut stream).expect("ok").is_none());
    assert_eq!(stream.len(), 3);

    // Concatenated frames decode in order.
    let mut stream = BytesMut::new();
    stream.extend_from_slice(&[0x7d, 0x00]);
    stream.extend_from_slice(&[0x60, 0x03, 9, 0, 0, 0]);
    stream.extend_from_slice(&[0x7d, 0x00]);
    let ops: Vec<u16> = std::iter::from_fn(|| decoder.decode(&mut stream).expect("ok"))
        .map(|raw| raw.opcode.0)
        .collect();
    assert_eq!(ops, vec![0x007d, 0x0360, 0x007d]);
    assert!(stream.is_empty());

    // Dynamic length smaller than its own header.
    let mut stream = BytesMut::from(&[0xfd_u8, 0x09, 3, 0][..]);
    assert!(matches!(
        decoder.decode(&mut stream),
        Err(ro_packets::PacketCodecError::MalformedHeader(_))
    ));
}

/// PKT-ITEM-01: item packet lengths match the 20260219 table and the capture.
#[test]
fn pkt_item_01_table_lengths() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;
    let expected = [
        (0x0add, PacketLength::Fixed(24)),
        (0x00a1, PacketLength::Fixed(6)),
        (0x009d, PacketLength::Fixed(19)),
        (0x0b41, PacketLength::Fixed(70)),
        (0x0b08, PacketLength::Dynamic),
        (0x0b0b, PacketLength::Fixed(4)),
        (0x01d7, PacketLength::Fixed(15)),
        (0x00af, PacketLength::Fixed(6)),
        (0x0b09, PacketLength::Dynamic),
        (0x0b39, PacketLength::Dynamic),
        (0x0a9b, PacketLength::Dynamic),
    ];
    for (op, len) in expected {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(len),
            "opcode {op:#06x}"
        );
    }
}

/// PKT-EQ-01: equipment packet lengths match the 20260219 table, the catalog and the capture.
#[test]
fn pkt_eq_01_table_lengths() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let table = RathenaPacketLengthTable;
    let ver = ro_core::TARGET_PACKETVER;
    let expected = [
        (0x0999, 11), // ZC_ACK_WEAR_EQUIP_V5
        (0x099a, 9),  // ZC_ACK_TAKEOFF_EQUIP_V5
        (0x0bae, 3),  // ZC_ACK_TAKEOFF_EQUIP_ALL
        (0x013c, 4),  // ZC_EQUIP_ARROW
        (0x013b, 4),  // ZC_ACTION_FAILURE
        (0x0291, 4),  // ZC_MSG
        (0x07e2, 8),  // ZC_MSG_VALUE
        (0x07e6, 8),  // ZC_MSG_SKILL
        (0x09cd, 8),  // ZC_MSG_COLOR
        (0x0983, 29), // ZC_MSG_STATE_CHANGE3
        (0x0196, 9),  // ZC_SC_END
        (0x0998, 8),  // CZ_REQ_WEAR_EQUIP_V5
        (0x00ab, 4),  // CZ_REQ_TAKEOFF_EQUIP
        (0x0bf5, 6),  // CZ_REQ_TAKEOFF_EQUIP_ALL
        (0x013a, 4),  // ZC_ATTACK_RANGE
    ];
    for (op, len) in expected {
        assert_eq!(
            table.packet_length(PacketOpcode(op), ver),
            Some(PacketLength::Fixed(len)),
            "opcode {op:#06x}"
        );
    }
}

/// PF-21: `0x080e` ZC_NOTIFY_HP_TO_GROUPM_R2 is 14 bytes (`u32 AID, i32 hp, i32 max_hp`), per
/// `reference/catalog/captures/equip_sequence.txt` line 267.
#[test]
fn pf21_party_hp_length() {
    use ro_packets::{PacketLength, PacketLengthTable};
    assert_eq!(
        RathenaPacketLengthTable.packet_length(PacketOpcode(0x080e), ro_core::TARGET_PACKETVER),
        Some(PacketLength::Fixed(14))
    );
}

/// PF-21: rows the captures misframed (catalog PF-21 table; `0x010e` after `@jlevel` and skill
/// points stalled the proxy log in `equip_sequence_3.txt` line 2967).
#[test]
fn pf21_capture_3_rows() {
    use ro_packets::{PacketLength, PacketLengthTable};
    let len =
        |op| RathenaPacketLengthTable.packet_length(PacketOpcode(op), ro_core::TARGET_PACKETVER);
    for (op, n) in [
        (0x010e, 11),
        (0x0441, 4),
        (0x00b6, 6),
        (0x0148, 8),
        (0x09cb, 17),
        (0x0b1a, 29),
        (0x00c0, 7),
        (0x019b, 10),
        (0x07fa, 8),
        (0x0139, 16),
    ] {
        assert_eq!(len(op), Some(PacketLength::Fixed(n)), "0x{op:04x}");
    }
    assert_eq!(len(0x00b4), Some(PacketLength::Dynamic));
}
