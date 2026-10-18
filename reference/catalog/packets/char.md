# Character Server Packets (`CH_*` / `HC_*`)

Target client version: `PACKETVER = 20260219`. Packet entries are sorted by ascending hexadecimal opcode.

## Raw 4-Byte `AccountId` Echo Policy

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 5) & `rathena` (`src/char/char_clif.cpp`, `chclif_parse_reqtoconnect`, `PACKETVER = 20260219`).
* **Handshake Framing:** Immediately after accepting `CH_ENTER` (`0x0065`), the character server writes the 4-byte little-endian `AccountId` directly to the TCP stream without a 2-byte packet opcode header.
* **Client Framing Behavior:** The official client reads these raw 4 bytes as an out-of-band stream prefix before switching back to standard 2-byte opcode framing for `HC_ACCEPT_ENTER` and character slot page packets.
* **Socket Handoff Policy:** Upon sending `HC_NOTIFY_ZONESVR` (`0x0ac5`), the character server keeps the TCP connection open until the client closes the socket (`Ok(0)` EOF).

## Packet Specifications

### `0x0ac5` `HC_NOTIFY_ZONESVR`

* **Opcode:** `0x0ac5`
* **Canonical Name:** `HC_NOTIFY_ZONESVR`
* **Direction:** Server to Client (Character Server)
* **Frame Length:** Fixed `156` bytes
* **Provenance:** `rathena` (`src/char/packets.hpp`, `src/char/char_clif.cpp`), `PACKETVER = 20260219` (`PACKETVER >= 20170315`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0ac5` (little-endian) |
| `2` | `4` | `u32` | `char_id` | Selected character ID (`CID`) |
| `6` | `16` | `[u8; 16]` | `map_name` | Null-terminated `.gat` map name (for example, `prontera.gat`) |
| `22` | `4` | `u32` | `ip` | Map server IPv4 address in network byte order |
| `26` | `2` | `u16` | `port` | Map server TCP port in little-endian byte order |
| `28` | `128` | `[u8; 128]` | `domain` | Zero-padded map server domain string |
