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
1. **Pinned Client Version & Packet Shuffling:**
   * The workspace pins `TARGET_PACKETVER` (`20260219`) as the single supported client version. Omit backwards compatibility for legacy packet versions.
   * For `PACKETVER = 20260219`, map server incoming packet opcodes are defined in `reference/rathena/src/map/clif_shuffle.hpp`. Definitions in `clif_shuffle.hpp` take precedence over `clif_packetdb.hpp`.
   * When evaluating rAthena C/C++ headers, evaluate `#if PACKETVER` blocks against `20260219` (`PACKETVER_MAIN_NUM = 20260219`) and disregard older branches.
2. **Canonical Core Opcodes for `20260219`:**
   * `CZ_ENTER`: `0x0436` (Fixed 23 bytes; account id at byte 2, char id at byte 6, auth code at byte 10, client tick at byte 14, 4 bytes padding at bytes 18..22, sex at byte 22).
   * `CZ_NOTIFY_ACTORINIT`: `0x007d` (Fixed 2 bytes).
   * `CZ_REQUEST_TIME`: `0x0360` (Fixed 6 bytes; client tick at byte 2).
   * `CZ_REQUEST_MOVE`: `0x035f` (Fixed 5 bytes; 3-byte packed coords at byte 2).
   * `CZ_CHANGE_DIRECTION`: `0x0361` (Fixed 5 bytes; head dir at byte 2, body dir at byte 4).
   * `CZ_REQUEST_ACT`: `0x0437` (Fixed 7 bytes; target id at byte 2, action type at byte 6).
   * `CZ_REQNAME`: `0x0368` (Fixed 6 bytes; entity id at byte 2).
   * `AC_ACCEPT_LOGIN`: `0x0ac4` (Dynamic; login acceptance with char server list).
   * `HC_NOTIFY_ZONESVR`: `0x0ac5` (Fixed 156 bytes; zone server transfer notification).
3. **Macro Offset Parsing:**
   * In `parseable_packet(opcode, length, handler, pos0, pos1, ...)`, trailing integer arguments specify exact byte offsets from byte 0 of the packet frame.
   * Decoders must use these exact offsets to identify field locations and any intermediate padding bytes.
4. **Protocol Extraction (No Standalone C++ Builds):**
   * Do not compile standalone C++ test programs against rAthena headers. The headers transitively pull in MySQL client libraries and internal subsystem headers.
   * Extract packet tables using the C preprocessor (`clang -E -DPACKETVER=20260219 ...`) or ripgrep queries over macro patterns.
5. **Opcode & Length Framing:**
   * Every Ragnarok Online packet starts with a 2-byte little-endian `PacketOpcode` (`u16`).
   * `RathenaPacketLengthTable` implements `PacketLengthTable` (`crates/contracts/src/packet.rs`), mapping each `(PacketOpcode, packetver)` to `PacketLength::Fixed(usize)` or `PacketLength::Dynamic`.
   * For `PacketLength::Dynamic`, bytes `2..4` contain a little-endian `u16` specifying the total frame byte length (including opcode and length field).
   * `PacketDecoder` implements `tokio_util::codec::Decoder`, reading from `BytesMut` and emitting `RawPacket { opcode, payload: Bytes }` once a complete frame has arrived.
6. **Graceful Unknown Opcode Handling:**
   * Clients frequently send auxiliary opcodes not yet used by the current phase.
   * As long as `PacketLengthTable` knows the frame length of the opcode from `reference/rathena/src/map/clif_packetdb.hpp` or `clif_shuffle.hpp`, `PacketDecoder` must consume the exact frame bytes and return `RawPacket`, and `PacketIntentParser::parse_intent` must return `Ok(None)` so the session worker logs a `tracing::debug!` message and keeps the TCP connection open.
7. **32-bit Item IDs:** Modern `PACKETVER` defines `MAX_ITEMID > UINT16_MAX`. Every item ID field on the wire and in contracts uses `u32`.

### 2.2 Coordinate & Direction Bit-Packing
Ragnarok Online packs map coordinates and directions into compact big-endian bitfields on the wire (reference: `reference/rathena/src/map/clif.cpp` `WBUFPOS` / `WBUFPOS2` and `reference/rust-ro/lib/packets/`):
1. **3-Byte Position (`WBUFPOS` / `RBUFPOS`):**
   * Used in packets such as `ZC_ACCEPT_ENTER` (`0x02eb`), `CZ_REQUEST_MOVE` (`0x035f`), `ZC_NOTIFY_STANDENTRY` (`0x09ff`), and `ZC_NOTIFY_NEWENTRY` (`0x09fe`).
   * Packs `(x: u16, y: u16, dir: u8)` (10 bits X, 10 bits Y, 4 bits direction) into 3 bytes:
     * `byte[0] = (x >> 2) as u8`
     * `byte[1] = (((x & 0x03) << 6) | ((y >> 4) & 0x3F)) as u8`
     * `byte[2] = (((y & 0x0F) << 4) | ((dir as u16) & 0x0F)) as u8`
2. **6-Byte Movement Pair (`WBUFPOS2` / `RBUFPOS2`):**
   * Used in movement broadcast packets (`ZC_NOTIFY_PLAYERMOVE` `0x0087`, `ZC_NOTIFY_MOVE` `0x09fd`).
   * Packs start `(x0, y0)`, destination `(x1, y1)`, and sub-cell offsets `(sx, sy)` (10 bits X0, 10 bits Y0, 10 bits X1, 10 bits Y1, 4 bits SX, 4 bits SY) into 6 bytes:
     * `byte[0] = (x0 >> 2) as u8`
     * `byte[1] = (((x0 & 0x03) << 6) | ((y0 >> 4) & 0x3F)) as u8`
     * `byte[2] = (((y0 & 0x0F) << 4) | ((x1 >> 6) & 0x0F)) as u8`
     * `byte[3] = (((x1 & 0x3F) << 2) | ((y1 >> 8) & 0x03)) as u8`
     * `byte[4] = (y1 & 0xFF) as u8`
     * `byte[5] = (((sx & 0x0F) << 4) | (sy & 0x0F)) as u8`

### 2.3 Wire Identity vs. Session Context
* Entry handshake packets (`CA_LOGIN`, `CH_ENTER`, `CZ_ENTER`) carry explicit credentials or `AccountId` / `CharId` fields in their wire payload.
* Subsequent in-session packets (`CH_SELECT_CHAR` `0x0066`, `CZ_NOTIFY_ACTORINIT` `0x007d`, `CZ_REQUEST_TIME` `0x0360`, `CZ_REQUEST_MOVE` `0x035f`, `CZ_CHANGE_DIRECTION` `0x0361`, `CZ_REQUEST_ACT` `0x0437`, `CZ_REQNAME` `0x0368`) **do not contain the sender's `AccountId` or `CharId` on the wire**.
* `PacketIntentParser` only extracts the fields physically present in `RawPacket::payload`. Session identity (`SessionId`, `AccountId`, `CharId`) is tracked and attached by the binary session worker (`session.rs`) after handshake authentication.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Callers:** `bins/login-server/src/session.rs`, `bins/char-server/src/session.rs`, and `bins/map-server/src/{session,service}.rs`.
* **Traits Implemented by `crates/packets`:**
  * `PacketLengthTable`: Evaluates frame size from opcode and `packetver`.
  * `PacketIntentParser`: Converts `RawPacket` into `Result<Option<ClientIntent>, PacketCodecError>`.
  * `EventEncoder`: Serializes `ServerEvent` into `Bytes` and `ScopedServerEvent` into `OutboundPacket`.
* **Isolation Boundary:** `crates/packets` has zero dependency on `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, or `ro-scripting`. It never touches `TcpStream`, channels, SQL, or ECS state.
