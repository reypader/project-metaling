# AGENTS.md: `crates/packets` Subagent (Binary Wire Protocol & Codecs)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/packets/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/packets/src/**` and `./crates/packets/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). If you need a new shared type, error variant, or enum in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & Wire Protocol Mechanics

`crates/packets` (`ro-packets`) owns the binary wire protocol translation between raw TCP byte streams and the high-level domain contracts (`ClientIntent`, `ServerEvent`, `ScopedServerEvent`, `OutboundPacket`) defined in `crates/contracts`.

### 2.1 Framing & `PACKETVER` Rules
1. **Target Client Version:** Evaluate all packet layouts against `TARGET_PACKETVER` (`20211103`) defined in `crates/contracts` and `reference/rathena/src/config/packets.hpp`. When inspecting `reference/rathena` C/C++ headers, evaluate `#if PACKETVER` blocks against `20211103` and ignore legacy branches.
2. **Opcode & Length Framing:**
   * Every Ragnarok Online packet starts with a 2-byte little-endian `PacketOpcode` (`u16`).
   * `RathenaPacketLengthTable` implements `PacketLengthTable` (`crates/contracts/src/packet.rs`), mapping each `(PacketOpcode, packetver)` to `PacketLength::Fixed(usize)` or `PacketLength::Dynamic`.
   * For `PacketLength::Dynamic`, bytes `2..4` contain a little-endian `u16` specifying the total frame byte length (including opcode and length field).
   * `PacketDecoder` implements `tokio_util::codec::Decoder`, reading from `BytesMut` and emitting `RawPacket { opcode, payload: Bytes }` once a complete frame has arrived.
3. **Graceful Unknown Opcode Handling:**
   * Clients frequently send auxiliary opcodes not yet used by the current phase.
   * As long as `PacketLengthTable` knows the frame length of the opcode from `reference/rathena/src/map/clif_packetdb.hpp`, `PacketDecoder` must consume the exact frame bytes and return `RawPacket`, and `PacketIntentParser::parse_intent` must return `Ok(None)` so the session worker logs a `tracing::debug!` message and keeps the TCP connection open.
4. **32-bit Item IDs:** Modern `PACKETVER` defines `MAX_ITEMID > UINT16_MAX`. Every item ID field on the wire and in contracts uses `u32`.

### 2.2 Coordinate & Direction Bit-Packing
Ragnarok Online packs map coordinates and directions into compact big-endian bitfields on the wire (reference: `reference/rathena/src/map/clif.cpp` `WBUFPOS` / `WBUFPOS2` and `reference/rust-ro/lib/packets/`):
1. **3-Byte Position (`WBUFPOS` / `RBUFPOS`):**
   * Used in packets such as `ZC_ACCEPT_ENTER` (`0x0073`), `CZ_REQUEST_MOVE` (`0x0085`), `ZC_NOTIFY_STANDENTRY`, and `ZC_NOTIFY_NEWENTRY`.
   * Packs `(x: u16, y: u16, dir: u8)` (10 bits X, 10 bits Y, 4 bits direction) into 3 bytes:
     * `byte[0] = (x >> 2) as u8`
     * `byte[1] = (((x & 0x03) << 6) | ((y >> 4) & 0x3F)) as u8`
     * `byte[2] = (((y & 0x0F) << 4) | ((dir as u16) & 0x0F)) as u8`
2. **6-Byte Movement Pair (`WBUFPOS2` / `RBUFPOS2`):**
   * Used in movement broadcast packets (`ZC_NOTIFY_PLAYERMOVE` `0x0087`, `ZC_NOTIFY_MOVE` `0x0086`).
   * Packs start `(x0, y0)`, destination `(x1, y1)`, and sub-cell offsets `(sx, sy)` (10 bits X0, 10 bits Y0, 10 bits X1, 10 bits Y1, 4 bits SX, 4 bits SY) into 6 bytes:
     * `byte[0] = (x0 >> 2) as u8`
     * `byte[1] = (((x0 & 0x03) << 6) | ((y0 >> 4) & 0x3F)) as u8`
     * `byte[2] = (((y0 & 0x0F) << 4) | ((x1 >> 6) & 0x0F)) as u8`
     * `byte[3] = (((x1 & 0x3F) << 2) | ((y1 >> 8) & 0x03)) as u8`
     * `byte[4] = (y1 & 0xFF) as u8`
     * `byte[5] = (((sx & 0x0F) << 4) | (sy & 0x0F)) as u8`

### 2.3 Wire Identity vs. Session Context
* Entry handshake packets (`CA_LOGIN`, `CH_ENTER`, `CZ_ENTER`) carry explicit credentials or `AccountId` / `CharId` fields in their wire payload.
* Subsequent in-session packets (`CH_SELECT_CHAR` `0x0066`, `CZ_NOTIFY_ACTORINIT` `0x007D`, `CZ_REQUEST_TIME` `0x007E`, `CZ_REQUEST_MOVE` `0x0085`, `CZ_CHANGE_DIRECTION` `0x009B`, `CZ_REQUEST_ACT` `0x0089`, `CZ_REQNAME` `0x0094`) **do not contain the sender's `AccountId` or `CharId` on the wire**.
* `PacketIntentParser` only extracts the fields physically present in `RawPacket::payload`. Session identity (`SessionId`, `AccountId`, `CharId`) is tracked and attached by the binary session worker (`session.rs`) after handshake authentication.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Callers:** `bins/login-server/src/session.rs`, `bins/char-server/src/session.rs`, and `bins/map-server/src/{session,service}.rs`.
* **Traits Implemented by `crates/packets`:**
  * `PacketLengthTable`: Evaluates frame size from opcode and `packetver`.
  * `PacketIntentParser`: Converts `RawPacket` into `Result<Option<ClientIntent>, PacketCodecError>`.
  * `EventEncoder`: Serializes `ServerEvent` into `Bytes` and `ScopedServerEvent` into `OutboundPacket`.
* **Isolation Boundary:** `crates/packets` has zero dependency on `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, or `ro-scripting`. It never touches `TcpStream`, channels, SQL, or ECS state.
