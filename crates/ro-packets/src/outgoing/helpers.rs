//! Helper functions for outgoing packet serialization.

/// Writes a string into a fixed-size byte buffer, padding with null bytes.
pub fn write_fixed_string<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let len = bytes.len().min(N);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}
