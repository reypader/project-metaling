use super::*;

#[test]
fn test_decode_fixed_packet() {
    let mut decoder = PacketDecoder::default();
    let mut buf = BytesMut::new();

    // 0x0436 is CZ_ENTER (23 bytes total: 2-byte opcode + 21-byte payload).
    buf.extend_from_slice(&[0x36, 0x04]);
    // Only 2 bytes in buffer; should yield None.
    assert_eq!(decoder.decode(&mut buf).unwrap(), None);

    // Append 21 payload bytes.
    let payload = [1u8; 21];
    buf.extend_from_slice(&payload);

    let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
    assert_eq!(packet.opcode, PacketOpcode(0x0436));
    assert_eq!(packet.payload.as_ref(), &payload);
    assert!(buf.is_empty());
}

#[test]
fn test_decode_dynamic_packet() {
    let mut decoder = PacketDecoder::default();
    let mut buf = BytesMut::new();

    // 0x0ac4 is AC_ACCEPT_LOGIN (dynamic).
    // Total len: 8 bytes (2 opcode + 2 len + 4 payload).
    buf.extend_from_slice(&[0xc4, 0x0a, 0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD]);

    let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
    assert_eq!(packet.opcode, PacketOpcode(0x0ac4));
    assert_eq!(packet.payload.as_ref(), &[0xAA, 0xBB, 0xCC, 0xDD]);
    assert!(buf.is_empty());
}

#[test]
fn test_decode_unhandled_known_packet_graceful() {
    let mut decoder = PacketDecoder::default();
    let mut buf = BytesMut::new();

    // 0x00b9 is NpcNextClicked (6 bytes fixed in clif_packetdb.hpp).
    buf.extend_from_slice(&[0xb9, 0x00, 0x01, 0x02, 0x03, 0x04]);

    let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
    assert_eq!(packet.opcode, PacketOpcode(0x00b9));
    assert_eq!(packet.payload.len(), 4);
    assert!(buf.is_empty());

    // Decodes into Unhandled frame rather than failing or dropping connection.
    let frame = decode_frame(&packet, TARGET_PACKETVER).unwrap();
    match frame {
        DecodedFrame::Unhandled { opcode, len } => {
            assert_eq!(opcode, PacketOpcode(0x00b9));
            assert_eq!(len, 6);
        }
        _ => panic!("Expected unhandled frame"),
    }
}

#[test]
fn test_decode_unknown_opcode_fails() {
    let mut decoder = PacketDecoder::default();
    let mut buf = BytesMut::new();

    buf.extend_from_slice(&[0xff, 0xff, 0x00, 0x00]);
    let err = decoder.decode(&mut buf).unwrap_err();
    match err {
        PacketCodecError::UnknownOpcode(op) => assert_eq!(op, PacketOpcode(0xffff)),
        _ => panic!("Expected UnknownOpcode error"),
    }
}
