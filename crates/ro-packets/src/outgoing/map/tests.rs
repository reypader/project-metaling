use super::*;

#[test]
fn test_encode_zc_accept_enter() {
    let pos = MapPosition {
        x: 156,
        y: 180,
        dir: 4,
    };
    let bytes = encode_zc_accept_enter(99999, pos, 5, 5);

    assert_eq!(bytes.len(), 13);
    assert_eq!(&bytes[0..2], &0x02ebu16.to_le_bytes());
    assert_eq!(&bytes[2..6], &99999u32.to_le_bytes());
    assert_eq!(bytes[9], 5);
    assert_eq!(bytes[10], 5);
    assert_eq!(&bytes[11..13], &0u16.to_le_bytes());
}

#[test]
fn test_encode_zc_notify_time() {
    let bytes = encode_zc_notify_time(123456);
    assert_eq!(bytes.len(), 6);
    assert_eq!(&bytes[0..2], &0x007fu16.to_le_bytes());
    assert_eq!(&bytes[2..6], &123456u32.to_le_bytes());
}

#[test]
fn test_encode_zc_par_change() {
    let bytes = encode_zc_par_change(10, 50);
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[0..2], &0x00b0u16.to_le_bytes());
    assert_eq!(&bytes[2..4], &10u16.to_le_bytes());
    assert_eq!(&bytes[4..8], &50i32.to_le_bytes());
}
