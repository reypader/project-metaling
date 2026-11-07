//! Client-to-server packets the tests send.

use ro_packets::pack_pos_dir;

use super::*;

pub fn cz_enter(n: u32) -> Vec<u8> {
    cz_enter_raw(account(n), char_id(n))
}

pub fn cz_enter_raw(account_id: u32, char_id: u32) -> Vec<u8> {
    let mut b = vec![0x36, 0x04];
    b.extend_from_slice(&account_id.to_le_bytes());
    b.extend_from_slice(&char_id.to_le_bytes());
    b.extend_from_slice(&12345u32.to_le_bytes());
    b.extend_from_slice(&100_000u32.to_le_bytes());
    b.extend_from_slice(&[0u8; 4]);
    b.push(1);
    b
}

pub fn cz_request_move(x: u16, y: u16) -> Vec<u8> {
    let mut b = vec![0x5f, 0x03];
    b.extend_from_slice(&pack_pos_dir(x, y, 0));
    b
}

pub fn cz_change_direction(head: u8, body: u8) -> Vec<u8> {
    vec![0x61, 0x03, head, 0, body]
}

pub fn cz_reqname(gid: u32) -> Vec<u8> {
    let mut b = vec![0x68, 0x03];
    b.extend_from_slice(&gid.to_le_bytes());
    b
}

pub fn cz_request_act(action: u8) -> Vec<u8> {
    let mut b = vec![0x37, 0x04];
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(action);
    b
}

pub fn cz_request_time(tick: u32) -> Vec<u8> {
    let mut b = vec![0x60, 0x03];
    b.extend_from_slice(&tick.to_le_bytes());
    b
}
pub fn chat(n: u32, message: &str) -> Vec<u8> {
    let text = format!("Char{n} : {message}");
    let len = u16::try_from(4 + text.len()).expect("short");
    let mut frame = vec![0xf3, 0x00];
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(text.as_bytes());
    frame
}
