//! Integration tests for packet framing, coordinate packing, and packet codecs.

use bytes::BytesMut;
use ro_contracts::ids::{
    AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, Sex,
};
use ro_contracts::packet::{ClientIntent, EventEncoder, PacketOpcode, ServerEvent};
use ro_packets::codec::PacketDecoder;
use ro_packets::coords::{pack_pos2, pack_pos_dir, unpack_pos2, unpack_pos_dir};
use ro_packets::incoming::{decode_frame, DecodedFrame};
use ro_packets::outgoing::{
    encode_ac_accept_login, encode_ac_refuse_login, encode_hc_notify_zonesvr,
    encode_zc_accept_enter, encode_zc_notify_act, encode_zc_notify_act_params,
    encode_zc_notify_newentry, encode_zc_notify_playermove, encode_zc_notify_time,
    encode_zc_par_change, ActParams, CharServerEntry, NewEntryParams, Phase0EventEncoder,
};
use tokio_util::codec::Decoder;

#[test]
fn test_stream_fragmentation_and_coalescing() {
    let mut decoder = PacketDecoder::default();
    let mut stream_buffer = BytesMut::new();

    // Prepare 3 packets:
    // 1. CZ_ENTER (0x0072, 19 bytes)
    let mut cz_enter_bytes = vec![0x72, 0x00];
    cz_enter_bytes.extend_from_slice(&2000001u32.to_le_bytes()); // account_id
    cz_enter_bytes.extend_from_slice(&150001u32.to_le_bytes());  // char_id
    cz_enter_bytes.extend_from_slice(&12345u32.to_le_bytes());   // login_id1
    cz_enter_bytes.extend_from_slice(&99999u32.to_le_bytes());   // client_tick
    cz_enter_bytes.push(1); // Sex::Male

    // 2. CZ_REQUEST_TIME (0x007e, 6 bytes)
    let mut cz_time_bytes = vec![0x7e, 0x00];
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
    assert_eq!(decoded_packets[0].opcode, PacketOpcode(0x0072));
    let frame1 = decode_frame(&decoded_packets[0], ro_contracts::TARGET_PACKETVER).unwrap();
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
    assert_eq!(decoded_packets[1].opcode, PacketOpcode(0x007e));
    let frame2 = decode_frame(&decoded_packets[1], ro_contracts::TARGET_PACKETVER).unwrap();
    assert_eq!(
        frame2,
        DecodedFrame::Intent(ClientIntent::RequestTime {
            client_tick: 100001,
        })
    );

    // Check packet 3
    assert_eq!(decoded_packets[2].opcode, PacketOpcode(0x007d));
    let frame3 = decode_frame(&decoded_packets[2], ro_contracts::TARGET_PACKETVER).unwrap();
    assert_eq!(
        frame3,
        DecodedFrame::Intent(ClientIntent::NotifyActorInit)
    );

    assert!(stream_buffer.is_empty());
}

#[test]
fn test_unhandled_packet_consumption() {
    let mut decoder = PacketDecoder::default();
    let mut stream = BytesMut::new();

    // 0x00bf is CZ_REQ_EMOTION (3 bytes fixed in clif_packetdb.hpp)
    stream.extend_from_slice(&[0xbf, 0x00, 0x07]);

    let raw = decoder.decode(&mut stream).unwrap().expect("packet expected");
    assert_eq!(raw.opcode, PacketOpcode(0x00bf));
    assert_eq!(raw.payload.as_ref(), &[0x07]);
    assert!(stream.is_empty());

    let frame = decode_frame(&raw, ro_contracts::TARGET_PACKETVER).unwrap();
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
    );
    let mut stream = BytesMut::from(login_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("AC_ACCEPT_LOGIN decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0069));
    assert_eq!(raw.payload_len(), 47 + 32 - 4);

    // 2. AC_REFUSE_LOGIN (23 bytes)
    let refuse_bytes = encode_ac_refuse_login(3, "0");
    let mut stream = BytesMut::from(refuse_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("AC_REFUSE_LOGIN decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x006a));
    assert_eq!(raw.payload_len(), 21);

    // 3. HC_NOTIFY_ZONESVR (28 bytes)
    let zone_bytes = encode_hc_notify_zonesvr(
        CharId(150001),
        "geffen.gat",
        [127, 0, 0, 1],
        5121,
    );
    let mut stream = BytesMut::from(zone_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("HC_NOTIFY_ZONESVR decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0071));
    assert_eq!(raw.payload_len(), 26);

    // 4. ZC_ACCEPT_ENTER (11 bytes)
    let accept_bytes = encode_zc_accept_enter(
        12345,
        MapPosition { x: 50, y: 75, dir: 2 },
        5,
        5,
    );
    let mut stream = BytesMut::from(accept_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_ACCEPT_ENTER decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0073));
    assert_eq!(raw.payload_len(), 9);

    // 5. ZC_NOTIFY_TIME (6 bytes)
    let time_bytes = encode_zc_notify_time(8888);
    let mut stream = BytesMut::from(time_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_NOTIFY_TIME decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x007f));
    assert_eq!(raw.payload_len(), 4);

    // 6. ZC_NOTIFY_NEWENTRY (53 bytes)
    let entry_params = NewEntryParams {
        gid: 150001,
        speed: 150,
        body_state: 0,
        health_state: 0,
        effect_state: 0,
        job: 1,
        head: 2,
        weapon: 0,
        accessory: 0,
        shield: 0,
        accessory2: 0,
        accessory3: 0,
        headpalette: 0,
        bodypalette: 0,
        head_dir: 0,
        guid: 0,
        gemblem_ver: 0,
        honor: 0,
        virtue: 0,
        is_pkmode_on: 0,
        sex: Sex::Female,
        pos: MapPosition { x: 100, y: 150, dir: 6 },
        x_size: 5,
        y_size: 5,
        clevel: 10,
    };
    let newentry_bytes = encode_zc_notify_newentry(&entry_params);
    let mut stream = BytesMut::from(newentry_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_NOTIFY_NEWENTRY decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x009d));
    assert_eq!(raw.payload_len(), 51);

    // 7. ZC_PAR_CHANGE (5 bytes)
    let par_bytes = encode_zc_par_change(5, 99);
    let mut stream = BytesMut::from(par_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_PAR_CHANGE decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x00be));
    assert_eq!(raw.payload_len(), 3);

    // 8. ZC_NOTIFY_PLAYERMOVE (12 bytes)
    let move_bytes = encode_zc_notify_playermove(10, 20, 30, 40, 5000);
    let mut stream = BytesMut::from(move_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_NOTIFY_PLAYERMOVE decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x0087));
    assert_eq!(raw.payload_len(), 10);

    // 9. ZC_NOTIFY_ACT (29 bytes)
    let act_bytes = encode_zc_notify_act(100, 200, 1000, 0, 0, 10, 1, 0, 0);
    let mut stream = BytesMut::from(act_bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().expect("ZC_NOTIFY_ACT decoded");
    assert_eq!(raw.opcode, PacketOpcode(0x008a));
    assert_eq!(raw.payload_len(), 27);

    // 10. ZC_NOTIFY_ACT via ActParams (29 bytes)
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
    let ver = ro_contracts::TARGET_PACKETVER;
    let mut decoder = PacketDecoder::default();

    // 1. AcceptEnter
    let event = ServerEvent::AcceptEnter {
        server_tick: 1111,
        pos: MapPosition { x: 50, y: 60, dir: 1 },
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0073));

    // 2. LoginAccepted
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
    assert_eq!(raw.opcode, PacketOpcode(0x0069));

    // 3. LoginRefused
    let event = ServerEvent::LoginRefused { error_code: 6 };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x006a));

    // 4. CharacterListLoaded (produces HC_ACCEPT_ENTER + HC_ACK_CHARINFO_PER_PAGE)
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
            last_pos: MapPosition { x: 150, y: 150, dir: 4 },
        }],
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw1 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw1.opcode, PacketOpcode(0x006b));
    let raw2 = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw2.opcode, PacketOpcode(0x099d));
    assert!(stream.is_empty());

    // 5. CharacterSelected
    let event = ServerEvent::CharacterSelected {
        char_id: CharId(150001),
        map_name: "prontera.gat".to_string(),
        map_host: [192, 168, 1, 10],
        map_port: 5121,
    };
    let bytes = encoder.encode_event(&event, ver).unwrap();
    let mut stream = BytesMut::from(bytes.as_ref());
    let raw = decoder.decode(&mut stream).unwrap().unwrap();
    assert_eq!(raw.opcode, PacketOpcode(0x0071));
}

#[test]
fn test_login_and_char_intent_decoding() {
    let ver = ro_contracts::TARGET_PACKETVER;

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
