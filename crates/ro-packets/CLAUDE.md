# ro-packets

Opcodes, lengths, and layouts live in `src/table.rs`, `src/incoming/`, `src/outgoing/`, and `reference/catalog/packets/`. Don't duplicate them here.

## Rules that are easy to get wrong

- **Shuffled map opcodes:** for `20260219`, client→map opcodes in rAthena `src/map/clif_shuffle.hpp` override `clif_packetdb.hpp`. If an opcode looks wrong, check the shuffle file first.
- **`parseable_packet(op, len, handler, pos0, pos1, …)`:** the trailing numbers are absolute byte offsets from the start of the frame, opcode included. Read fields at exactly those offsets and skip any gaps (padding).
- **Dynamic-length frames:** bytes `2..4` hold the total frame length as a little-endian u16, including the opcode and the length field itself.
- **Unknown but known-length opcodes:** consume the frame, have `parse_intent` return `Ok(None)`, log at `debug!`, and keep the session open. Don't disconnect.
- **Bit-packed positions are big-endian bitfields** (`WBUFPOS` 3 bytes, `WBUFPOS2` 6 bytes). See `src/coords.rs`; its tests include a rust-ro reference vector.
- **Item IDs are u32** everywhere (`MAX_ITEMID > u16::MAX` for modern clients).
- Parsers extract only fields that are physically on the wire. In-session packets carry no AccountId/CharId (see `bins/CLAUDE.md`).
