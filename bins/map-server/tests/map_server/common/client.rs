//! A packet-level test client and readers for the frames it receives.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::BytesMut;
use ro_core::TARGET_PACKETVER;
use ro_packets::{unpack_pos2, PacketDecoder, PacketOpcode, RathenaPacketLengthTable, RawPacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_util::codec::Decoder;

use super::*;

pub struct Client {
    stream: TcpStream,
    decoder: PacketDecoder<RathenaPacketLengthTable>,
    buf: BytesMut,
    /// Frames read but not yet consumed by `wait_for`.
    pending: Vec<RawPacket>,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.expect("connect"),
            decoder: PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER),
            buf: BytesMut::with_capacity(4096),
            pending: Vec::new(),
        }
    }

    pub async fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.expect("write");
        self.stream.flush().await.expect("flush");
    }

    pub async fn read_exact_bytes(&mut self, n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        tokio::time::timeout(Duration::from_secs(2), self.stream.read_exact(&mut out))
            .await
            .expect("timed out reading burst")
            .expect("read burst");
        out
    }

    /// Reads until EOF or `timeout`; returns the raw bytes and whether EOF was seen.
    pub async fn read_until_eof(&mut self, timeout: Duration) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let mut chunk = [0u8; 512];
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return (out, false);
            }
            match tokio::time::timeout(left, self.stream.read(&mut chunk)).await {
                Ok(Ok(0)) | Ok(Err(_)) => return (out, true),
                Ok(Ok(n)) => out.extend_from_slice(&chunk[..n]),
                Err(_) => return (out, false),
            }
        }
    }

    async fn fill(&mut self, wait: Duration) -> bool {
        match tokio::time::timeout(wait, self.stream.read_buf(&mut self.buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => false,
            Ok(Ok(_)) => {
                while let Some(frame) = self.decoder.decode(&mut self.buf).expect("clean framing") {
                    self.pending.push(frame);
                }
                true
            }
            Err(_) => true,
        }
    }

    /// Returns every frame that arrives within `window` (plus anything already buffered).
    pub async fn collect(&mut self, window: Duration) -> Vec<RawPacket> {
        let deadline = tokio::time::Instant::now() + window;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() || !self.fill(left).await {
                break;
            }
        }
        std::mem::take(&mut self.pending)
    }

    /// Waits for the first frame matching `pred`; earlier non-matching frames are kept.
    pub async fn wait_for(
        &mut self,
        timeout: Duration,
        pred: impl Fn(&RawPacket) -> bool,
    ) -> Option<RawPacket> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(i) = self.pending.iter().position(&pred) {
                return Some(self.pending.remove(i));
            }
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() || !self.fill(left).await {
                return None;
            }
        }
    }

    /// Consumes a login's frames between the friends list and the first `0x007d` (MOTD, `0x0091`,
    /// status push, ... up to the closing `0x0ade`; specs/map-entry-settings §5) and returns them.
    pub async fn read_login_burst(&mut self) -> Vec<RawPacket> {
        let last = self
            .wait_opcode(0x0ade, Duration::from_secs(2))
            .await
            .expect("login burst ends with 0x0ade");
        let mut burst = std::mem::take(&mut self.pending);
        burst.push(last);
        burst
    }

    pub async fn wait_opcode(&mut self, opcode: u16, timeout: Duration) -> Option<RawPacket> {
        self.wait_for(timeout, |f| f.opcode == PacketOpcode(opcode))
            .await
    }

    /// Half-closes: sends EOF but keeps reading.
    pub async fn close_write(&mut self) {
        let _ = self.stream.shutdown().await;
    }

    /// Closes the write side and drops the connection.
    pub async fn close(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

/// Full frame length (opcode + payload) for assertions on packet sizes.
pub fn frame_len(frame: &RawPacket) -> usize {
    frame.payload.len() + 2 + if is_dynamic(frame.opcode.0) { 2 } else { 0 }
}

/// Opcodes with a u16 length field after the opcode, among those the tests inspect.
fn is_dynamic(op: u16) -> bool {
    (0x09fd..=0x09ff).contains(&op) || matches!(op, 0x0201 | 0x010f | 0x0b8d)
}

/// u32 at a FRAME offset (offset 0 = opcode). Handles the dynamic length prefix.
pub fn frame_u32(frame: &RawPacket, offset: usize) -> u32 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    let p = &frame.payload[offset - base..offset - base + 4];
    u32::from_le_bytes(p.try_into().unwrap())
}

pub fn frame_u16(frame: &RawPacket, offset: usize) -> u16 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    let p = &frame.payload[offset - base..offset - base + 2];
    u16::from_le_bytes(p.try_into().unwrap())
}

pub fn frame_u8(frame: &RawPacket, offset: usize) -> u8 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    frame.payload[offset - base]
}

/// AID of a unit entry packet (0x09fd/0x09fe/0x09ff).
pub fn entry_aid(frame: &RawPacket) -> u32 {
    frame_u32(frame, 5)
}

pub fn entry_gid(frame: &RawPacket) -> u32 {
    frame_u32(frame, 9)
}

/// (x0, y0, x1, y1, sx, sy) of a 0x0087 frame.
pub fn playermove(frame: &RawPacket) -> (u16, u16, u16, u16, u8, u8) {
    let mv: [u8; 6] = frame.payload[4..10].try_into().unwrap();
    unpack_pos2(&mv)
}

/// (map name, x, y) of a `0x0091` frame.
pub fn mapmove(frame: &RawPacket) -> (String, u16, u16) {
    let name = &frame.payload[0..16];
    let end = name.iter().position(|&b| b == 0).unwrap_or(16);
    (
        String::from_utf8_lossy(&name[..end]).into_owned(),
        frame_u16(frame, 18),
        frame_u16(frame, 20),
    )
}

/// A unit entry packet (0x09fd/0x09fe/0x09ff).
pub fn is_entry(frame: &RawPacket) -> bool {
    (0x09fd..=0x09ff).contains(&frame.opcode.0)
}

/// Opcodes of `frames`, in order.
pub fn ops(frames: &[RawPacket]) -> Vec<u16> {
    frames.iter().map(|f| f.opcode.0).collect()
}

/// Enters as `n` and returns everything in the 150 ms after the `0x007d`.
pub async fn enter(server: &TestServer, n: u32) -> (Client, Vec<RawPacket>) {
    let mut client = Client::connect(server.addr).await;
    client.send(&cz_enter(n)).await;
    // ZC_AID, 0x0b18, ZC_ACCEPT_ENTER, the empty friends list, then the login burst
    // (specs/map-entry-settings).
    client.read_exact_bytes(23 + 4).await;
    client.read_login_burst().await;
    client.send(&[0x7d, 0x00]).await;
    let frames = client.collect(Duration::from_millis(150)).await;
    (client, frames)
}
