//! Stream framing: fragmented, coalesced and unhandled packets (PKT-02).

use super::*;

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
