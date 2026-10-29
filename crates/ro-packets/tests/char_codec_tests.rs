//! Char-select entry status values (spec `specs/traits/spec.md` PKT-CHAR-01; catalog
//! `packets/char.md` "Status fields in `CHARACTER_INFO`" and "Captured select-screen values").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::BytesMut;
use ro_core::{
    AccountId, CharId, CharacterLooks, CharacterSelectStats, CharacterSummary, MapPosition, Sex,
};
use ro_packets::encode_character_info;

fn summary(select_stats: CharacterSelectStats) -> CharacterSummary {
    CharacterSummary {
        char_id: CharId(150_011),
        account_id: AccountId(2_000_000),
        slot: 8,
        name: "test".into(),
        sex: Sex::Male,
        job_class: 4252,
        base_level: 99,
        job_level: 1,
        hair_style: 1,
        hair_color: 0,
        last_map: "prontera".into(),
        last_pos: MapPosition {
            x: 150,
            y: 150,
            dir: 0,
        },
        delete_date: None,
        looks: CharacterLooks::default(),
        select_stats,
    }
}

fn entry(select_stats: CharacterSelectStats) -> Vec<u8> {
    let mut buf = BytesMut::new();
    encode_character_info(&mut buf, &summary(select_stats));
    assert_eq!(buf.len(), 175);
    buf.to_vec()
}

fn i64_at(e: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(e[at..at + 8].try_into().unwrap())
}

fn i16_at(e: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(e[at..at + 2].try_into().unwrap())
}

/// PKT-CHAR-01: the captured DK entry (`rathena_traits_sequence.txt` 1346 `0x006b`, 1356 `0x0b72`):
/// jobpoint 1325, sppoint 11, HP 8251/11575, SP 315/467, STR..LUK 1, exp 0.
#[test]
fn pkt_char_01_entry_carries_the_saved_status() {
    let e = entry(CharacterSelectStats {
        base_exp: 0,
        job_exp: 0,
        hp: 8251,
        max_hp: 11575,
        sp: 315,
        max_sp: 467,
        status_point: 1325,
        skill_point: 11,
        stats: [1; 6],
    });
    assert_eq!((i64_at(&e, 4), i64_at(&e, 16)), (0, 0), "exp, jobexp");
    assert_eq!(i16_at(&e, 48), 1325, "jobpoint");
    assert_eq!(
        (
            i64_at(&e, 50),
            i64_at(&e, 58),
            i64_at(&e, 66),
            i64_at(&e, 74)
        ),
        (8251, 11575, 315, 467)
    );
    assert_eq!(i16_at(&e, 94), 11, "sppoint");
    assert_eq!(e[132..138], [1; 6]);

    // Exp, and distinct stats in their order.
    let e = entry(CharacterSelectStats {
        base_exp: 5_000_000_000,
        job_exp: 7,
        stats: [2, 3, 4, 5, 6, 7],
        ..CharacterSelectStats::default()
    });
    assert_eq!((i64_at(&e, 4), i64_at(&e, 16)), (5_000_000_000, 7));
    assert_eq!(e[132..138], [2, 3, 4, 5, 6, 7]);
}

/// PKT-CHAR-01: points, SP and MaxSP cap at 32767 and stats at 255; HP and MaxHP don't cap.
#[test]
fn pkt_char_01_caps() {
    let e = entry(CharacterSelectStats {
        hp: 100_000,
        max_hp: 1_000_000,
        sp: 40_000,
        max_sp: 50_000,
        status_point: 40_000,
        skill_point: u16::MAX,
        stats: [300, 255, 254, 1, 0, 999],
        ..CharacterSelectStats::default()
    });
    assert_eq!(i16_at(&e, 48), 32_767, "jobpoint");
    assert_eq!(i16_at(&e, 94), 32_767, "sppoint");
    assert_eq!(
        (
            i64_at(&e, 50),
            i64_at(&e, 58),
            i64_at(&e, 66),
            i64_at(&e, 74)
        ),
        (100_000, 1_000_000, 32_767, 32_767)
    );
    assert_eq!(e[132..138], [255, 255, 254, 1, 0, 255]);
}
