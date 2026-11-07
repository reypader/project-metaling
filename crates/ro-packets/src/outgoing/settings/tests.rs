use super::*;

#[test]
fn shortcut_key_list_is_271_bytes_with_tab() {
    let tab1 = encode_zc_shortcut_key_list_empty(1);
    assert_eq!(tab1.len(), 271);
    assert_eq!(&tab1[..5], &[0x20, 0x0b, 0x00, 0x01, 0x00]);
    assert!(tab1[5..].iter().all(|b| *b == 0));
}

#[test]
fn repute_info_is_69_bytes() {
    let bytes = encode_zc_repute_info_default();
    assert_eq!(bytes.len(), 69);
    assert_eq!(&bytes[..5], &[0x8d, 0x0b, 0x45, 0x00, 0x01]);
    assert_eq!(&bytes[5..13], &1u64.to_le_bytes());
    assert!(bytes[13..21].iter().all(|b| *b == 0));
}
