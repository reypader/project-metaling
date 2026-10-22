//! Domain unit and integration tests for map entry protocol codecs,
//! framing, and event serialization under `PACKETVER = 20260219`.
//!
//! Traceable against scenarios in `map_entry_handshake_test_spec.md`:
//! - `PKT-LEN-001`: Length table mapping for `ZC_INVENTORY_EXPANSION_INFO` (0x0b18)
//! - `PKT-ENC-001`: Wire serialization of `encode_zc_aid`
//! - `PKT-ENC-002`: Wire serialization of `encode_zc_inventory_expansion_info`
//! - `PKT-ENC-003`: Wire serialization of `encode_zc_accept_enter`
//! - `PKT-ENC-004`: Atomic map entry burst encoding for `ServerEvent::AcceptEnter`
//! - `PKT-ENC-005`: Discrete event encoding for `ServerEvent::Aid` & `ServerEvent::InventoryExpansionInfo`
//! - `PKT-DEC-001`: Intent parsing for `CZ_NOTIFY_ACTORINIT` and `CZ_REQUEST_TIME`

use bytes::{Bytes, BytesMut};
use ro_contracts::ids::{AccountId, MapPosition};
use ro_contracts::packet::{
    ClientIntent, EventEncoder, PacketLength, PacketLengthTable, PacketOpcode, RawPacket,
    ServerEvent,
};
use ro_packets::codec::PacketDecoder;
use ro_packets::coords::pack_pos_dir;
use ro_packets::incoming::{decode_frame, DecodedFrame};
use ro_packets::outgoing::{
    encode_zc_accept_enter, encode_zc_aid, encode_zc_inventory_expansion_info, ModernEventEncoder,
};
use ro_packets::table::RathenaPacketLengthTable;
use tokio_util::codec::Decoder;

/// Scenario `PKT-LEN-001`: Length Table Mapping for `ZC_INVENTORY_EXPANSION_INFO`
#[test]
fn test_opcode_length_table_zc_inventory_expansion_info() {
    let table = RathenaPacketLengthTable;
    assert_eq!(
        table.packet_length(PacketOpcode::ZC_INVENTORY_EXPANSION_INFO, 20260219),
        Some(PacketLength::Fixed(4))
    );
    assert_eq!(
        table.packet_length(PacketOpcode::ZC_AID, 20260219),
        Some(PacketLength::Fixed(6))
    );
    assert_eq!(
        table.packet_length(PacketOpcode::ZC_ACCEPT_ENTER, 20260219),
        Some(PacketLength::Fixed(13))
    );
}

/// Scenario `PKT-ENC-001`: Serialization of `encode_zc_aid`
#[test]
fn test_encode_zc_aid_layout() {
    let bytes = encode_zc_aid(AccountId(2000005));
    assert_eq!(bytes.len(), 6);
    assert_eq!(&bytes[0..2], &[0x83, 0x02]);
    assert_eq!(&bytes[2..6], &2000005u32.to_le_bytes());
}

/// Scenario `PKT-ENC-002`: Serialization of `encode_zc_inventory_expansion_info`
#[test]
fn test_encode_zc_inventory_expansion_info_layout() {
    let bytes0 = encode_zc_inventory_expansion_info(0);
    assert_eq!(bytes0.len(), 4);
    assert_eq!(bytes0.as_ref(), &[0x18, 0x0b, 0x00, 0x00]);

    let bytes10 = encode_zc_inventory_expansion_info(10);
    assert_eq!(bytes10.len(), 4);
    assert_eq!(bytes10.as_ref(), &[0x18, 0x0b, 0x0a, 0x00]);

    let bytes_neg = encode_zc_inventory_expansion_info(-1);
    assert_eq!(bytes_neg.len(), 4);
    assert_eq!(bytes_neg.as_ref(), &[0x18, 0x0b, 0xff, 0xff]);
}

/// Scenario `PKT-ENC-003`: Serialization of `encode_zc_accept_enter`
#[test]
fn test_encode_zc_accept_enter_layout() {
    let bytes = encode_zc_accept_enter(
        54321,
        MapPosition {
            x: 120,
            y: 240,
            dir: 4,
        },
        5,
        5,
    );
    assert_eq!(bytes.len(), 13);
    assert_eq!(&bytes[0..2], &[0xeb, 0x02]);
    assert_eq!(&bytes[2..6], &54321u32.to_le_bytes());
    assert_eq!(&bytes[6..9], &pack_pos_dir(120, 240, 4));
    assert_eq!(bytes[9], 5);
    assert_eq!(bytes[10], 5);
    assert_eq!(&bytes[11..13], &[0x00, 0x00]);
}

/// Scenario `PKT-ENC-004`: Atomic Map Entry Burst Encoding for `ServerEvent::AcceptEnter`
#[test]
fn test_accept_enter_event_encodes_23_byte_atomic_burst() {
    let encoder = ModernEventEncoder;
    let event = ServerEvent::AcceptEnter {
        account_id: AccountId(2000001),
        server_tick: 8888,
        pos: MapPosition {
            x: 100,
            y: 200,
            dir: 1,
        },
    };
    let bytes = encoder.encode_event(&event, 20260219).unwrap();
    assert_eq!(bytes.len(), 23);

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, 20260219);
    let mut stream = BytesMut::from(bytes.as_ref());

    let pkt1 = decoder.decode(&mut stream).unwrap().expect("ZC_AID");
    assert_eq!(pkt1.opcode, PacketOpcode::ZC_AID);
    assert_eq!(pkt1.payload.len(), 4);
    assert_eq!(&pkt1.payload[..], &2000001u32.to_le_bytes());

    let pkt2 = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_INVENTORY_EXPANSION_INFO");
    assert_eq!(pkt2.opcode, PacketOpcode::ZC_INVENTORY_EXPANSION_INFO);
    assert_eq!(pkt2.payload.len(), 2);
    assert_eq!(&pkt2.payload[..], &[0x00, 0x00]);

    let pkt3 = decoder
        .decode(&mut stream)
        .unwrap()
        .expect("ZC_ACCEPT_ENTER");
    assert_eq!(pkt3.opcode, PacketOpcode::ZC_ACCEPT_ENTER);
    assert_eq!(pkt3.payload.len(), 11);
    assert_eq!(&pkt3.payload[0..4], &8888u32.to_le_bytes());
    assert_eq!(&pkt3.payload[4..7], &pack_pos_dir(100, 200, 1));
    assert_eq!(pkt3.payload[7], 5);
    assert_eq!(pkt3.payload[8], 5);
    assert_eq!(&pkt3.payload[9..11], &[0x00, 0x00]);
    assert!(stream.is_empty());
}

/// Scenario `PKT-ENC-005`: Discrete Event Encoding for `ServerEvent::Aid` and `InventoryExpansionInfo`
#[test]
fn test_discrete_aid_and_expansion_events() {
    let encoder = ModernEventEncoder;
    let aid_bytes = encoder
        .encode_event(
            &ServerEvent::Aid {
                account_id: AccountId(2000001),
            },
            20260219,
        )
        .unwrap();
    assert_eq!(aid_bytes.len(), 6);
    assert_eq!(&aid_bytes[0..2], &[0x83, 0x02]);
    assert_eq!(&aid_bytes[2..6], &2000001u32.to_le_bytes());

    let inv_bytes = encoder
        .encode_event(
            &ServerEvent::InventoryExpansionInfo { expansion_size: 5 },
            20260219,
        )
        .unwrap();
    assert_eq!(inv_bytes.len(), 4);
    assert_eq!(&inv_bytes[0..2], &[0x18, 0x0b]);
    assert_eq!(&inv_bytes[2..4], &5i16.to_le_bytes());
}

/// Scenario `PKT-DEC-001`: Intent Parsing for `CZ_NOTIFY_ACTORINIT` and `CZ_REQUEST_TIME`
#[test]
fn test_decode_actorinit_and_request_time() {
    let frame_init = decode_frame(
        &RawPacket::new(PacketOpcode(0x007d), Bytes::new()),
        20260219,
    )
    .unwrap();
    assert_eq!(
        frame_init,
        DecodedFrame::Intent(ClientIntent::NotifyActorInit)
    );

    let frame_time = decode_frame(
        &RawPacket::new(
            PacketOpcode(0x0360),
            Bytes::copy_from_slice(&20000u32.to_le_bytes()),
        ),
        20260219,
    )
    .unwrap();
    assert_eq!(
        frame_time,
        DecodedFrame::Intent(ClientIntent::RequestTime { client_tick: 20000 })
    );
}
