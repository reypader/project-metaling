use std::io;

use bytes::{Bytes, BytesMut};
use tokio_util::codec::Encoder;

/// Simple packet encoder converting raw byte buffers into outbound network frames.
#[derive(Debug, Default, Clone, Copy)]
pub struct PacketEncoder;

impl Encoder<Bytes> for PacketEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(&item);
        Ok(())
    }
}
