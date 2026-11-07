//! Engine-foundation map-server scenarios (spec §2): BIN-01..09, BIN-SRV-01..04/06, BIN-CFG-01.
//! In-memory repo + injected 200x200 grid; characters `n` are account 200000n / char 15000n.

use std::time::Duration;

use crate::common::*;
use map_server::load_walkability;
use ro_core::{CharId, MapPosition};
use ro_packets::RawPacket;

use crate::common::LONG;

mod entry_and_movement;
mod server_and_config;

const SHORT: Duration = Duration::from_millis(300);

fn entries_about(frames: &[RawPacket], n: u32) -> usize {
    frames
        .iter()
        .filter(|f| is_entry(f) && entry_aid(f) == account(n))
        .count()
}

// ---------- BIN-09 ----------

fn act_type(frame: &RawPacket) -> u8 {
    frame_u8(frame, 29)
}

// ---------- BIN-CFG-01 ----------

fn gat_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"GRAT\x01\x02".to_vec();
    b.extend_from_slice(&width.to_le_bytes());
    b.extend_from_slice(&height.to_le_bytes());
    for _ in 0..width * height {
        b.extend_from_slice(&[0u8; 20]);
    }
    b
}

// ---------- M6 review follow-ups ----------

fn burst_tick(burst: &[u8]) -> u32 {
    u32::from_le_bytes(burst[12..16].try_into().unwrap())
}

fn burst_cell(burst: &[u8]) -> (u16, u16) {
    let pos: [u8; 3] = burst[16..19].try_into().unwrap();
    let (x, y, _) = ro_packets::unpack_pos_dir(&pos);
    (x, y)
}
