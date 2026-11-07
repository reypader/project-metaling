use super::*;

#[test]
fn status_is_44_bytes_and_saturates() {
    let status = StatusWindow {
        status_point: 40_000,
        stats: [300, 2, 3, 4, 5, 6],
        costs: [0, 2, 2, 2, 2, 2],
        aspd: 590,
        ..StatusWindow::default()
    };
    let bytes = encode_zc_status(&status);
    assert_eq!(bytes.len(), 44);
    assert_eq!(&bytes[..6], &[0xbd, 0x00, 0xff, 0x7f, 0xff, 0x00]);
    assert_eq!(&bytes[40..44], &[0x4e, 0x02, 0x00, 0x00]);
}
