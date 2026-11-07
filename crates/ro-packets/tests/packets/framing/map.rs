//! Map-server packets: intents, coordinates and the event encoder.

use super::*;

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
            select_stats: Default::default(),
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
            select_stats: Default::default(),
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
