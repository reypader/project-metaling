//! Item, floor item and chat packets (spec PKT-ITEM-02..07, PKT-CHAT-01/02).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use ro_core::{
    ClientIntent, FloorItemAppearance, ItemEntry, ItemOption, ServerEvent, TARGET_PACKETVER,
};
use ro_packets::{
    decode_frame, DecodedFrame, EventEncoder, ModernEventEncoder, PacketOpcode, RawPacket,
};

fn encode(event: ServerEvent) -> Vec<u8> {
    ModernEventEncoder
        .encode_event(&event, TARGET_PACKETVER)
        .expect("encodes")
        .to_vec()
}

fn decode(op: u16, payload: &[u8]) -> ClientIntent {
    let raw = RawPacket::new(PacketOpcode(op), Bytes::copy_from_slice(payload));
    match decode_frame(&raw, TARGET_PACKETVER).expect("decodes") {
        DecodedFrame::Intent(intent) => intent,
        other => panic!("{op:#06x} not decoded: {other:?}"),
    }
}

fn plain_entry(index: u16, item_id: u32, item_type: u8, amount: u16) -> ItemEntry {
    ItemEntry {
        index,
        item_id,
        item_type,
        amount,
        location: 0,
        wear_state: 0,
        cards: [0; 4],
        expire_time: 0,
        bind_on_equip_type: 0,
        look: 0,
        options: [ItemOption::default(); 5],
        refine: 0,
        grade: 0,
        identified: true,
        damaged: false,
        favorite: false,
    }
}

/// PKT-ITEM-02: `0x0362` -> PickUpItem, `0x0363` -> DropItem (wire index kept).
#[test]
fn pkt_item_02_decodes_pickup_and_drop() {
    assert_eq!(
        decode(0x0362, &[0x41, 0, 0, 0]),
        ClientIntent::PickUpItem { floor_id: 0x41 }
    );
    assert_eq!(
        decode(0x0363, &[0x0c, 0x00, 0x02, 0x00]),
        ClientIntent::DropItem {
            index: 12,
            amount: 2
        }
    );
    let short = RawPacket::new(PacketOpcode(0x0363), Bytes::from_static(&[1, 0]));
    assert!(decode_frame(&short, TARGET_PACKETVER).is_err());
}

/// PKT-ITEM-03: `0x0add` encodes the captured payload byte for byte.
#[test]
fn pkt_item_03_item_fall_entry_matches_capture() {
    let bytes = encode(ServerEvent::FloorItemDropped {
        item: FloorItemAppearance {
            id: 0x41,
            item_id: 909,
            item_type: 3,
            identified: true,
            x: 157,
            y: 331,
            sub_x: 9,
            sub_y: 9,
            amount: 1,
            show_drop_effect: false,
            drop_effect_mode: 0,
        },
    });
    let captured: [u8; 24] = [
        0xdd, 0x0a, 0x41, 0x00, 0x00, 0x00, 0x8d, 0x03, 0x00, 0x00, 0x03, 0x00, 0x01, 0x9d, 0x00,
        0x4b, 0x01, 0x09, 0x09, 0x01, 0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(bytes, captured);

    let effect = encode(ServerEvent::FloorItemDropped {
        item: FloorItemAppearance {
            show_drop_effect: true,
            drop_effect_mode: 3,
            ..FloorItemAppearance {
                id: 2,
                item_id: 1,
                item_type: 3,
                identified: true,
                x: 1,
                y: 1,
                sub_x: 3,
                sub_y: 3,
                amount: 1,
                show_drop_effect: false,
                drop_effect_mode: 0,
            }
        },
    });
    assert_eq!(&effect[21..24], &[1, 3, 0]);
}

/// PKT-ITEM-04: `0x0b41` success matches the captured first 32 payload bytes; failure form.
#[test]
fn pkt_item_04_pickup_ack() {
    let bytes = encode(ServerEvent::ItemAdded {
        item: plain_entry(5, 909, 3, 1),
    });
    assert_eq!(bytes.len(), 70);
    assert_eq!(&bytes[0..2], &[0x41, 0x0b]);
    let captured: [u8; 32] = [
        0x05, 0x00, 0x01, 0x00, 0x8d, 0x03, 0x00, 0x00, 0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03, 0x00,
    ];
    assert_eq!(&bytes[2..34], &captured);
    assert!(bytes[34..].iter().all(|&b| b == 0), "{bytes:02x?}");

    let mut full = plain_entry(7, 1201, 5, 1);
    full.location = 0x2;
    full.cards = [4001, 0, 0, u32::MAX];
    full.expire_time = 0x0102_0304;
    full.bind_on_equip_type = 2;
    full.options[0] = ItemOption {
        id: 0x11,
        value: -2,
        param: 9,
    };
    full.favorite = true;
    full.damaged = true;
    full.look = 0x0a0b;
    full.refine = 7;
    full.grade = 2;
    let b = encode(ServerEvent::ItemAdded { item: full });
    assert_eq!(&b[2..6], &[7, 0, 1, 0]);
    assert_eq!(&b[10..12], &[1, 1], "identified, damaged");
    assert_eq!(&b[12..16], &4001u32.to_le_bytes());
    assert_eq!(&b[24..28], &u32::MAX.to_le_bytes());
    assert_eq!(&b[28..34], &[2, 0, 0, 0, 5, 0], "location, type, result");
    assert_eq!(&b[34..38], &0x0102_0304u32.to_le_bytes());
    assert_eq!(&b[38..40], &[2, 0]);
    assert_eq!(&b[40..45], &[0x11, 0, 0xfe, 0xff, 9]);
    assert_eq!(b[65], 1, "favorite");
    assert_eq!(&b[66..70], &[0x0b, 0x0a, 7, 2]);

    let fail = encode(ServerEvent::ItemAddFailed { result: 6 });
    let mut expected = vec![0u8; 70];
    expected[0..2].copy_from_slice(&[0x41, 0x0b]);
    expected[2] = 2;
    expected[33] = 6;
    assert_eq!(fail, expected);
}

/// PKT-ITEM-05: list packet layouts and the captured lengths 39 / 277 / 209.
#[test]
fn pkt_item_05_inventory_lists() {
    assert_eq!(
        encode(ServerEvent::InventoryStart),
        vec![0x08, 0x0b, 6, 0, 0, 0]
    );
    assert_eq!(encode(ServerEvent::InventoryEnd), vec![0x0b, 0x0b, 0, 0]);
    assert_eq!(encode(ServerEvent::EquipSwitchList), vec![0x9b, 0x0a, 4, 0]);

    let normal = encode(ServerEvent::InventoryNormalItems {
        items: vec![plain_entry(4, 23484, 2, 1)],
    });
    assert_eq!(normal.len(), 39);
    let captured: [u8; 16] = [
        0x09, 0x0b, 39, 0, 0x00, 0x04, 0x00, 0xbc, 0x5b, 0x00, 0x00, 0x02, 0x01, 0x00, 0, 0,
    ];
    assert_eq!(&normal[..16], &captured);
    assert_eq!(normal[38], 1, "flag: identified");

    let mut knife = plain_entry(2, 1201, 5, 1);
    knife.location = 2;
    knife.wear_state = 2;
    let equips = encode(ServerEvent::InventoryEquipItems {
        items: vec![knife.clone(), knife.clone(), knife.clone(), knife.clone()],
    });
    assert_eq!(equips.len(), 277);
    let captured: [u8; 17] = [
        0x39, 0x0b, 0x15, 0x01, 0x00, 0x02, 0x00, 0xb1, 0x04, 0x00, 0x00, 0x05, 0x02, 0x00, 0x00,
        0x00, 0x02,
    ];
    assert_eq!(&equips[..17], &captured);
    let three = encode(ServerEvent::InventoryEquipItems {
        items: vec![knife.clone(), knife.clone(), knife.clone()],
    });
    assert_eq!(three.len(), 209);

    let mut detailed = knife;
    detailed.look = 0x0102;
    detailed.bind_on_equip_type = 1;
    detailed.refine = 4;
    detailed.grade = 1;
    detailed.damaged = true;
    detailed.favorite = true;
    let e = encode(ServerEvent::InventoryEquipItems {
        items: vec![detailed],
    });
    let entry = &e[5..];
    assert_eq!(&entry[35..39], &[1, 0, 0x02, 0x01], "bind type, sprite");
    assert_eq!(entry[39], 0, "option count");
    assert_eq!(&entry[65..68], &[4, 1, 0b111], "refine, grade, flags");

    let mut fav = plain_entry(3, 501, 0, 5);
    fav.favorite = true;
    fav.identified = false;
    let n = encode(ServerEvent::InventoryNormalItems { items: vec![fav] });
    assert_eq!(n[5 + 33], 0b10, "favorite without identified");
}

/// PKT-ITEM-06: `0x009d` (19) and `0x00a1` (6).
#[test]
fn pkt_item_06_item_entry_and_disappear() {
    let bytes = encode(ServerEvent::FloorItemEntered {
        item: FloorItemAppearance {
            id: 0x41,
            item_id: 909,
            item_type: 3,
            identified: true,
            x: 157,
            y: 331,
            sub_x: 9,
            sub_y: 12,
            amount: 3,
            show_drop_effect: true,
            drop_effect_mode: 2,
        },
    });
    assert_eq!(
        bytes,
        vec![
            0x9d, 0x00, 0x41, 0, 0, 0, 0x8d, 0x03, 0, 0, 0x01, 0x9d, 0x00, 0x4b, 0x01, 0x03, 0x00,
            9, 12
        ]
    );
    assert_eq!(
        encode(ServerEvent::FloorItemVanished { id: 0x41 }),
        vec![0xa1, 0x00, 0x41, 0, 0, 0]
    );
}

/// PKT-ITEM-07: `0x00af`.
#[test]
fn pkt_item_07_throw_ack() {
    assert_eq!(
        encode(ServerEvent::ItemThrown {
            index: 12,
            amount: 0
        }),
        vec![0xaf, 0x00, 12, 0, 0, 0]
    );
    assert_eq!(
        encode(ServerEvent::ItemThrown {
            index: 5,
            amount: 300
        }),
        vec![0xaf, 0x00, 5, 0, 0x2c, 0x01]
    );
}

/// The pickup animation: `0x08c8` with src, target and type 1; everything else 0 (captured).
#[test]
fn pkt_item_pickup_action_matches_capture() {
    let bytes = encode(ServerEvent::EntityAction {
        source_id: 0x001e_8480,
        target_id: 0x41,
        action_type: 1,
        action_tick: 0,
    });
    let mut expected = vec![0u8; 34];
    expected[0..2].copy_from_slice(&[0xc8, 0x08]);
    expected[2..6].copy_from_slice(&[0x80, 0x84, 0x1e, 0x00]);
    expected[6] = 0x41;
    expected[29] = 1;
    assert_eq!(bytes, expected);
}

/// PKT-CHAT-01: `0x00f3` decodes with and without a trailing NUL; the captured frame.
#[test]
fn pkt_chat_01_decodes_global_message() {
    let captured = b"testtest : @item knife";
    assert_eq!(
        decode(0x00f3, captured),
        ClientIntent::Chat {
            text: captured.to_vec()
        }
    );
    assert_eq!(
        decode(0x00f3, b"testtest : hi\0"),
        ClientIntent::Chat {
            text: b"testtest : hi".to_vec()
        }
    );
    assert_eq!(decode(0x00f3, b""), ClientIntent::Chat { text: Vec::new() });
}

/// PKT-CHAT-02: `0x008d` and `0x008e`, NUL-terminated; long text is cut to 255 bytes.
#[test]
fn pkt_chat_02_encodes_chat() {
    let text = b"a : hi".to_vec();
    let bytes = encode(ServerEvent::PublicChat {
        gid: 2_000_000,
        text: text.clone(),
    });
    let mut expected = vec![0x8d, 0x00, 15, 0x00, 0x80, 0x84, 0x1e, 0x00];
    expected.extend_from_slice(&text);
    expected.push(0);
    assert_eq!(bytes, expected);

    let bytes = encode(ServerEvent::SelfChat { text: text.clone() });
    let mut expected = vec![0x8e, 0x00, 11, 0x00];
    expected.extend_from_slice(&text);
    expected.push(0);
    assert_eq!(bytes, expected);

    let long = encode(ServerEvent::SelfChat {
        text: vec![b'x'; 400],
    });
    assert_eq!(long.len(), 4 + 255 + 1);
    assert_eq!(u16::from_le_bytes([long[2], long[3]]), 260);
    assert_eq!(long.last(), Some(&0));
    let long = encode(ServerEvent::PublicChat {
        gid: 1,
        text: vec![b'x'; 400],
    });
    assert_eq!(long.len(), 8 + 255 + 1);
}
