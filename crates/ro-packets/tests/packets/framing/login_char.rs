//! Login and char-server packets: intents, serializers and a captured login sequence.

use super::*;

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
                    is_boss: 0,
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
