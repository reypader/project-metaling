//! Movement, direction, action and name-request intents (PKT-03, 04).

use super::*;

// ---------- PKT-03 ----------

#[test]
fn pkt_03_request_move_decodes() {
    let payload = pack_pos_dir(104, 100, 0);
    assert_eq!(
        parse(0x035f, &payload).expect("decodes"),
        Some(ClientIntent::RequestMove {
            dest_x: 104,
            dest_y: 100
        })
    );
    let edge = pack_pos_dir(1023, 1023, 0);
    assert_eq!(
        parse(0x035f, &edge).expect("decodes"),
        Some(ClientIntent::RequestMove {
            dest_x: 1023,
            dest_y: 1023
        })
    );
}

#[test]
fn pkt_03_change_direction_decodes() {
    assert_eq!(
        parse(0x0361, &[2, 0, 4]).expect("decodes"),
        Some(ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4
        })
    );
}

#[test]
fn pkt_03_change_direction_out_of_range_is_invalid_field() {
    for payload in [[3u8, 0, 4], [0, 0, 8], [0, 0, 255], [255, 0, 0]] {
        assert!(
            matches!(
                parse(0x0361, &payload),
                Err(PacketCodecError::InvalidField { .. })
            ),
            "{payload:?} must be rejected"
        );
    }
}

#[test]
fn pkt_03_change_direction_ignores_gap_byte() {
    // rAthena reads head_dir as one byte at offset 2; offset 3 is never read.
    assert_eq!(
        parse(0x0361, &[2, 0xab, 4]).expect("gap byte ignored"),
        Some(ClientIntent::ChangeDirection {
            head_dir: 2,
            body_dir: 4
        })
    );
}

#[test]
fn pkt_03_truncated_move_and_direction() {
    assert!(matches!(
        parse(0x035f, &[0, 0]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
    assert!(matches!(
        parse(0x0361, &[0, 0]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
}

// ---------- PKT-04 ----------

#[test]
fn pkt_04_request_act_decodes_and_passes_unknown_types() {
    let mut payload = 2_000_002u32.to_le_bytes().to_vec();
    payload.push(2);
    assert_eq!(
        parse(0x0437, &payload).expect("decodes"),
        Some(ClientIntent::RequestAction {
            target_id: 2_000_002,
            action_type: 2
        })
    );
    payload[4] = 99;
    assert_eq!(
        parse(0x0437, &payload).expect("decodes"),
        Some(ClientIntent::RequestAction {
            target_id: 2_000_002,
            action_type: 99
        })
    );
}

#[test]
fn pkt_04_reqname_decodes() {
    assert_eq!(
        parse(0x0368, &2_000_001u32.to_le_bytes()).expect("decodes"),
        Some(ClientIntent::RequestEntityName {
            entity_id: 2_000_001
        })
    );
}

#[test]
fn pkt_04_truncated_act_and_reqname() {
    assert!(matches!(
        parse(0x0437, &[1, 2, 3, 4]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
    assert!(matches!(
        parse(0x0368, &[1, 2, 3]),
        Err(PacketCodecError::TruncatedPayload { .. })
    ));
}
