//! Helper functions for outgoing packet serialization.

/// Writes a string into a fixed-size C-string field: at most `N - 1` bytes, then NUL padding,
/// so the field always ends with a NUL (rAthena `safestrncpy`).
pub fn write_fixed_string<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let len = bytes.len().min(N.saturating_sub(1));
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}
