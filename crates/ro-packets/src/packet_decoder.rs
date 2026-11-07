//! Tokio framing codec for Ragnarok Online TCP stream.
//!
//! Handles 2-byte little-endian opcode inspection, length table lookup,
//! fixed vs variable length packet framing, and graceful unknown packet consumption.

use crate::{
    PacketCodecError, PacketLength, PacketLengthTable, PacketOpcode, RawPacket, MAX_PACKET_SIZE,
};
use bytes::BytesMut;
use ro_core::TARGET_PACKETVER;
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
            packetver: TARGET_PACKETVER,
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
    pub fn decode_frame(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<DecodedFrame>, PacketCodecError> {
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

#[cfg(test)]
mod tests;
