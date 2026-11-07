//! Engine-foundation packet scenarios PKT-03..PKT-13 (spec §3).
//! Offsets follow `reference/catalog/packets/map_units.md`.

use bytes::{Bytes, BytesMut};
use ro_core::{
    AccountId, CharId, ClientIntent, MapPosition, ServerEvent, Sex, UnitAppearance, UnitLooks,
    UnitObjectType, TARGET_PACKETVER,
};
use ro_packets::{
    is_implemented_outbound_opcode, pack_pos2, pack_pos_dir, unpack_pos2, unpack_pos_dir,
    EventEncoder, ModernEventEncoder, PacketCodecError, PacketDecoder, PacketIntentParser,
    PacketOpcode, Phase0IntentParser, RathenaPacketLengthTable, RawPacket,
};
use tokio_util::codec::Decoder;

mod coords;
mod intents;
mod npc_mob;
mod unit_events;

const VER: u32 = TARGET_PACKETVER;

fn parse(opcode: u16, payload: &[u8]) -> Result<Option<ClientIntent>, PacketCodecError> {
    let raw = RawPacket::new(PacketOpcode(opcode), Bytes::copy_from_slice(payload));
    Phase0IntentParser.parse_intent(&raw, VER)
}

fn encode(event: &ServerEvent) -> Bytes {
    ModernEventEncoder
        .encode_event(event, VER)
        .unwrap_or_else(|err| panic!("{event:?} should encode: {err}"))
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}
fn i16_at(b: &[u8], off: usize) -> i16 {
    i16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn str_at(b: &[u8], off: usize, len: usize) -> String {
    let field = &b[off..off + len];
    let end = field.iter().position(|&c| c == 0).unwrap_or(len);
    String::from_utf8(field[..end].to_vec()).expect("utf8 name")
}

/// Asserts the frame decodes as exactly one packet of `opcode` through the length table.
fn assert_frames_cleanly(bytes: &Bytes, opcode: u16) {
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, VER);
    let mut buf = BytesMut::from(bytes.as_ref());
    let raw = decoder
        .decode(&mut buf)
        .expect("frame decodes")
        .expect("whole frame present");
    assert_eq!(raw.opcode, PacketOpcode(opcode));
    assert!(
        buf.is_empty(),
        "table length must equal encoded length for {opcode:#06x}"
    );
}

fn alice() -> UnitAppearance {
    UnitAppearance {
        object_type: UnitObjectType::Pc,
        gid: 2_000_001,
        char_id: Some(CharId(150_001)),
        name: "Alice".into(),
        job: 4001,
        sex: Sex::Female,
        hair_style: 7,
        hair_color: 3,
        head_dir: 2,
        pos: MapPosition {
            x: 100,
            y: 140,
            dir: 6,
        },
        speed: 112,
        dead_sit: 2,
        clevel: 42,
        is_boss: 0,
        effect_state: 0,
        looks: UnitLooks::default(),
    }
}

// ---------- PKT-NPC (specs/npc-spawning) ----------

fn kafra() -> UnitAppearance {
    UnitAppearance {
        object_type: UnitObjectType::Npc,
        gid: 110_000_005,
        char_id: None,
        name: "Kafra Employee#prt1_with_a_long_tail".into(),
        job: 117,
        sex: Sex::Female,
        hair_style: 0,
        hair_color: 0,
        head_dir: 0,
        pos: MapPosition {
            x: 152,
            y: 326,
            dir: 4,
        },
        speed: 200,
        dead_sit: 0,
        clevel: 0,
        is_boss: 0,
        effect_state: 0x4,
        looks: UnitLooks::default(),
    }
}

// ---------- PKT-MOB (specs/mob-spawning) ----------

fn poring() -> UnitAppearance {
    UnitAppearance {
        object_type: UnitObjectType::Mob,
        gid: 110_000_042,
        char_id: None,
        name: "Elusive Wraith Dead (Nightmare)".into(),
        job: 1002,
        sex: Sex::Female,
        hair_style: 0,
        hair_color: 0,
        head_dir: 0,
        pos: MapPosition {
            x: 100,
            y: 200,
            dir: 0,
        },
        speed: 150,
        dead_sit: 0,
        clevel: 5,
        is_boss: 2,
        effect_state: 0,
        looks: UnitLooks::default(),
    }
}
