//! Inventory, floor item and chat packet encoders. Layouts: `reference/catalog/packets/
//! inventory.md`, `ground_items.md` and `chat.md`.

use bytes::{BufMut, Bytes, BytesMut};
use ro_core::{FloorItemAppearance, ItemEntry, ItemOption};

use crate::PacketOpcode;

/// Inventory type 0: the character's own inventory.
const INV_TYPE_INVENTORY: u8 = 0;
/// `0x0b09` entry size.
const NORMAL_ITEM_SIZE: usize = 34;
/// `0x0b39` entry size.
const EQUIP_ITEM_SIZE: usize = 68;
/// Longest chat text before the NUL (rAthena `CHAT_SIZE_MAX - 1`).
const MAX_CHAT_TEXT: usize = 255;

/// Writes the u16 length of a dynamic frame at bytes 2..4.
fn finish_dynamic(mut buf: BytesMut) -> Bytes {
    let len = u16::try_from(buf.len()).unwrap_or(u16::MAX).to_le_bytes();
    buf[2..4].copy_from_slice(&len);
    buf.freeze()
}

fn put_cards(buf: &mut BytesMut, cards: &[u32; 4]) {
    for card in cards {
        buf.put_u32_le(*card);
    }
}

fn put_options(buf: &mut BytesMut, options: &[ItemOption; 5]) {
    for option in options {
        if option.id == 0 {
            buf.put_bytes(0, 5);
        } else {
            buf.put_i16_le(option.id);
            buf.put_i16_le(option.value);
            buf.put_u8(option.param);
        }
    }
}

/// `ZC_INVENTORY_START` (0x0b08) for the inventory: type 0, empty name. 6 bytes.
pub fn encode_zc_inventory_start() -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_INVENTORY_START.raw());
    buf.put_u16_le(6);
    buf.put_u8(INV_TYPE_INVENTORY);
    buf.put_u8(0);
    buf.freeze()
}

/// `ZC_NORMAL_ITEMLIST` (0x0b09): `5 + 34 × n` bytes.
pub fn encode_zc_normal_itemlist(items: &[ItemEntry]) -> Bytes {
    let mut buf = BytesMut::with_capacity(5 + NORMAL_ITEM_SIZE * items.len());
    buf.put_u16_le(PacketOpcode::ZC_NORMAL_ITEMLIST.raw());
    buf.put_u16_le(0);
    buf.put_u8(INV_TYPE_INVENTORY);
    for item in items {
        buf.put_u16_le(item.index);
        buf.put_u32_le(item.item_id);
        buf.put_u8(item.item_type);
        buf.put_u16_le(item.amount);
        buf.put_u32_le(item.location);
        put_cards(&mut buf, &item.cards);
        buf.put_u32_le(item.expire_time);
        buf.put_u8(u8::from(item.identified) | u8::from(item.favorite) << 1);
    }
    finish_dynamic(buf)
}

/// `ZC_EQUIPMENT_ITEMLIST` (0x0b39): `5 + 68 × n` bytes.
pub fn encode_zc_equipment_itemlist(items: &[ItemEntry]) -> Bytes {
    let mut buf = BytesMut::with_capacity(5 + EQUIP_ITEM_SIZE * items.len());
    buf.put_u16_le(PacketOpcode::ZC_EQUIPMENT_ITEMLIST.raw());
    buf.put_u16_le(0);
    buf.put_u8(INV_TYPE_INVENTORY);
    for item in items {
        buf.put_u16_le(item.index);
        buf.put_u32_le(item.item_id);
        buf.put_u8(item.item_type);
        buf.put_u32_le(item.location);
        buf.put_u32_le(item.wear_state);
        put_cards(&mut buf, &item.cards);
        buf.put_u32_le(item.expire_time);
        buf.put_u16_le(item.bind_on_equip_type);
        buf.put_u16_le(item.look);
        buf.put_u8(0); // option count: the client counts them itself
        put_options(&mut buf, &item.options);
        buf.put_u8(item.refine);
        buf.put_u8(item.grade);
        buf.put_u8(
            u8::from(item.identified) | u8::from(item.damaged) << 1 | u8::from(item.favorite) << 2,
        );
    }
    finish_dynamic(buf)
}

/// `ZC_INVENTORY_END` (0x0b0b) for the inventory. 4 bytes.
pub fn encode_zc_inventory_end() -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_INVENTORY_END.raw());
    buf.put_u8(INV_TYPE_INVENTORY);
    buf.put_u8(0);
    buf.freeze()
}

/// The empty equip-switch list (0x0a9b). 4 bytes.
pub fn encode_zc_equipswitch_list_empty() -> Bytes {
    let mut buf = BytesMut::with_capacity(4);
    buf.put_u16_le(PacketOpcode::ZC_EQUIPSWITCH_LIST.raw());
    buf.put_u16_le(4);
    buf.freeze()
}

/// `ZC_ITEM_PICKUP_ACK` (0x0b41), 70 bytes. `None` is the failure form: index 2, count 0,
/// `result`, every other byte 0.
pub fn encode_zc_item_pickup_ack(item: Option<&ItemEntry>, result: u8) -> Bytes {
    let mut buf = BytesMut::with_capacity(70);
    buf.put_u16_le(PacketOpcode::ZC_ITEM_PICKUP_ACK.raw());
    let Some(item) = item else {
        buf.put_u16_le(2);
        buf.put_bytes(0, 29);
        buf.put_u8(result);
        buf.put_bytes(0, 36);
        return buf.freeze();
    };
    buf.put_u16_le(item.index);
    buf.put_u16_le(item.amount);
    buf.put_u32_le(item.item_id);
    buf.put_u8(u8::from(item.identified));
    buf.put_u8(u8::from(item.damaged));
    put_cards(&mut buf, &item.cards);
    buf.put_u32_le(item.location);
    buf.put_u8(item.item_type);
    buf.put_u8(result);
    buf.put_u32_le(item.expire_time);
    buf.put_u16_le(item.bind_on_equip_type);
    put_options(&mut buf, &item.options);
    buf.put_u8(u8::from(item.favorite));
    buf.put_u16_le(item.look);
    buf.put_u8(item.refine);
    buf.put_u8(item.grade);
    buf.freeze()
}

/// `ZC_ITEM_THROW_ACK` (0x00af): `amount` 0 means refused. 6 bytes.
pub fn encode_zc_item_throw_ack(index: u16, amount: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_ITEM_THROW_ACK.raw());
    buf.put_u16_le(index);
    buf.put_u16_le(amount);
    buf.freeze()
}

/// `ZC_ITEM_FALL_ENTRY` (0x0add): an item fell in view. 24 bytes.
pub fn encode_zc_item_fall_entry(item: &FloorItemAppearance) -> Bytes {
    let mut buf = BytesMut::with_capacity(24);
    buf.put_u16_le(PacketOpcode::ZC_ITEM_FALL_ENTRY5.raw());
    buf.put_u32_le(item.id);
    buf.put_u32_le(item.item_id);
    buf.put_u16_le(u16::from(item.item_type));
    buf.put_u8(u8::from(item.identified));
    buf.put_u16_le(item.x);
    buf.put_u16_le(item.y);
    buf.put_u8(item.sub_x);
    buf.put_u8(item.sub_y);
    buf.put_u16_le(item.amount);
    buf.put_u8(u8::from(item.show_drop_effect));
    buf.put_u16_le(item.drop_effect_mode);
    buf.freeze()
}

/// `ZC_ITEM_ENTRY` (0x009d): an item already on the floor came into view. 19 bytes.
pub fn encode_zc_item_entry(item: &FloorItemAppearance) -> Bytes {
    let mut buf = BytesMut::with_capacity(19);
    buf.put_u16_le(PacketOpcode::ZC_ITEM_ENTRY.raw());
    buf.put_u32_le(item.id);
    buf.put_u32_le(item.item_id);
    buf.put_u8(u8::from(item.identified));
    buf.put_u16_le(item.x);
    buf.put_u16_le(item.y);
    buf.put_u16_le(item.amount);
    buf.put_u8(item.sub_x);
    buf.put_u8(item.sub_y);
    buf.freeze()
}

/// `ZC_ITEM_DISAPPEAR` (0x00a1). 6 bytes.
pub fn encode_zc_item_disappear(id: u32) -> Bytes {
    let mut buf = BytesMut::with_capacity(6);
    buf.put_u16_le(PacketOpcode::ZC_ITEM_DISAPPEAR.raw());
    buf.put_u32_le(id);
    buf.freeze()
}

fn chat_text(text: &[u8]) -> &[u8] {
    &text[..text.len().min(MAX_CHAT_TEXT)]
}

/// `ZC_NOTIFY_CHAT` (0x008d): public chat from `gid`, NUL-terminated.
pub fn encode_zc_notify_chat(gid: u32, text: &[u8]) -> Bytes {
    let text = chat_text(text);
    let mut buf = BytesMut::with_capacity(9 + text.len());
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_CHAT.raw());
    buf.put_u16_le(0);
    buf.put_u32_le(gid);
    buf.put_slice(text);
    buf.put_u8(0);
    finish_dynamic(buf)
}

/// `ZC_NOTIFY_PLAYERCHAT` (0x008e): a line to self, NUL-terminated.
pub fn encode_zc_notify_playerchat(text: &[u8]) -> Bytes {
    let text = chat_text(text);
    let mut buf = BytesMut::with_capacity(5 + text.len());
    buf.put_u16_le(PacketOpcode::ZC_NOTIFY_PLAYERCHAT.raw());
    buf.put_u16_le(0);
    buf.put_slice(text);
    buf.put_u8(0);
    finish_dynamic(buf)
}
