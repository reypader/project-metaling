//! Tokio framing codec for Ragnarok Online TCP stream.
//!
//! Handles 2-byte little-endian opcode inspection, length table lookup,
//! fixed vs variable length packet framing, and graceful unknown packet consumption.

use bytes::{Bytes, BytesMut};
use ro_contracts::packet::{
    PacketCodecError, PacketLength, PacketLengthTable, PacketOpcode, RawPacket, MAX_PACKET_SIZE,
};
use tokio_util::codec::Decoder;

use crate::incoming::{decode_frame, DecodedFrame};
use crate::table::RathenaPacketLengthTable;

/// Stream decoder that frames raw network bytes into `RawPacket` instances.
#[derive(Debug, Clone)]
pub struct PacketDecoder<T = RathenaPacketLengthTable> {
    length_table: T,
    packetver: u32,
}

impl PacketDecoder<RathenaPacketLengthTable> {
    /// Creates a new decoder using the default rAthena length table for `TARGET_PACKETVER`.
    pub fn new_default() -> Self {
        Self {
            length_table: RathenaPacketLengthTable,
            packetver: ro_contracts::TARGET_PACKETVER,
        }
    }
}

impl<T: PacketLengthTable> PacketDecoder<T> {
    /// Creates a new decoder with a specific length table and packetver.
    pub fn new(length_table: T, packetver: u32) -> Self {
        Self {
            length_table,
            packetver,
        }
    }

    /// Accessor for the active packetver.
    pub fn packetver(&self) -> u32 {
        self.packetver
    }

    /// Decodes next frame directly into a high-level `DecodedFrame`.
    ///
    /// Consumes the raw frame and returns `DecodedFrame::Unhandled` for unhandled opcodes.
    pub fn decode_frame(&mut self, src: &mut BytesMut) -> Result<Option<DecodedFrame>, PacketCodecError> {
        let raw = match self.decode(src)? {
            Some(pkt) => pkt,
            None => return Ok(None),
        };

        Ok(Some(decode_frame(&raw, self.packetver)?))
    }
}

impl Default for PacketDecoder<RathenaPacketLengthTable> {
    fn default() -> Self {
        Self::new_default()
    }
}

impl<T: PacketLengthTable> Decoder for PacketDecoder<T> {
    type Item = RawPacket;
    type Error = PacketCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            return Ok(None);
        }

        let opcode_raw = u16::from_le_bytes([src[0], src[1]]);
        let opcode = PacketOpcode(opcode_raw);

        let packet_length = match self.length_table.packet_length(opcode, self.packetver) {
            Some(len) => len,
            None => {
                // If the opcode is absent from the packet database, fail with UnknownOpcode.
                return Err(PacketCodecError::UnknownOpcode(opcode));
            }
        };

        match packet_length {
            PacketLength::Fixed(len) => {
                if len < 2 {
                    return Err(PacketCodecError::MalformedHeader(format!(
                        "Fixed packet length {len} for opcode {opcode} is invalid"
                    )));
                }

                if len > MAX_PACKET_SIZE {
                    return Err(PacketCodecError::PacketTooLarge {
                        opcode,
                        length: len,
                        max: MAX_PACKET_SIZE,
                    });
                }

                if src.len() < len {
                    // Frame not complete yet. Wait for additional network bytes.
                    src.reserve(len - src.len());
                    return Ok(None);
                }

                let frame = src.split_to(len).freeze();
                let payload = frame.slice(2..);
                Ok(Some(RawPacket::new(opcode, payload)))
            }
            PacketLength::Dynamic => {
                if src.len() < 4 {
                    // Need 4 bytes to read opcode and total length header.
                    src.reserve(4 - src.len());
                    return Ok(None);
                }

                let total_len = u16::from_le_bytes([src[2], src[3]]) as usize;
                if total_len < 4 {
                    return Err(PacketCodecError::MalformedHeader(format!(
                        "Dynamic packet total length {total_len} for opcode {opcode} must be at least 4 bytes"
                    )));
                }

                if total_len > MAX_PACKET_SIZE {
                    return Err(PacketCodecError::PacketTooLarge {
                        opcode,
                        length: total_len,
                        max: MAX_PACKET_SIZE,
                    });
                }

                if src.len() < total_len {
                    // Frame not complete yet. Wait for additional network bytes.
                    src.reserve(total_len - src.len());
                    return Ok(None);
                }

                let frame = src.split_to(total_len).freeze();
                // Payload excludes the 2-byte opcode and 2-byte length header.
                let payload = frame.slice(4..);
                Ok(Some(RawPacket::new(opcode, payload)))
            }
        }
    }
}

/// Simple packet encoder converting raw byte buffers into outbound network frames.
#[derive(Debug, Default, Clone, Copy)]
pub struct PacketEncoder;

impl tokio_util::codec::Encoder<Bytes> for PacketEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(&item);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_fixed_packet() {
        let mut decoder = PacketDecoder::default();
        let mut buf = BytesMut::new();

        // 0x0072 is CZ_ENTER (19 bytes total: 2-byte opcode + 17-byte payload).
        buf.extend_from_slice(&[0x72, 0x00]);
        // Only 2 bytes in buffer; should yield None.
        assert_eq!(decoder.decode(&mut buf).unwrap(), None);

        // Append 17 payload bytes.
        let payload = [1u8; 17];
        buf.extend_from_slice(&payload);

        let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
        assert_eq!(packet.opcode, PacketOpcode(0x0072));
        assert_eq!(packet.payload.as_ref(), &payload);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_dynamic_packet() {
        let mut decoder = PacketDecoder::default();
        let mut buf = BytesMut::new();

        // 0x0069 is AC_ACCEPT_LOGIN (dynamic).
        // Total len: 8 bytes (2 opcode + 2 len + 4 payload).
        buf.extend_from_slice(&[0x69, 0x00, 0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD]);

        let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
        assert_eq!(packet.opcode, PacketOpcode(0x0069));
        assert_eq!(packet.payload.as_ref(), &[0xAA, 0xBB, 0xCC, 0xDD]);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_unhandled_known_packet_graceful() {
        let mut decoder = PacketDecoder::default();
        let mut buf = BytesMut::new();

        // 0x009f is CZ_ITEM_PICKUP (6 bytes fixed in clif_packetdb.hpp).
        buf.extend_from_slice(&[0x9f, 0x00, 0x01, 0x02, 0x03, 0x04]);

        let packet = decoder.decode(&mut buf).unwrap().expect("packet expected");
        assert_eq!(packet.opcode, PacketOpcode(0x009f));
        assert_eq!(packet.payload.len(), 4);
        assert!(buf.is_empty());

        // Decodes into Unhandled frame rather than failing or dropping connection.
        let frame = decode_frame(&packet, ro_contracts::TARGET_PACKETVER).unwrap();
        match frame {
            DecodedFrame::Unhandled { opcode, len } => {
                assert_eq!(opcode, PacketOpcode(0x009f));
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
}
