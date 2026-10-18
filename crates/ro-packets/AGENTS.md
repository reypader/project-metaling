# `crates/ro-packets` Module Rules (Binary Wire Protocol & Codecs)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-packets.md` (`ro-packets` custom subagent).
* **Write Scope:** `./crates/ro-packets/**` exclusively.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever opcode definitions, shuffled packet tables, or struct layouts from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate directly with `ro-bins` and `ro-tools` via subagent messaging when integrating packet codecs with server session workers or proxy tools.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-packets` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit protocol framing and edge cases before completion.
* **Red-Green Testing & Spec Collaboration:** Collaborate with `spec-writer` to define packet decoding, frame validation, bit-packing, and event encoding test scenarios supporting high-level bin-observable scenarios. Author or update unit and framing tests upfront (red phase) before writing production code changes.

---

## 2. Wire Protocol Specifications (`PACKETVER = 20260219`)

`crates/ro-packets` translates between raw TCP byte streams and the domain contracts (`ClientIntent`, `ServerEvent`, `ScopedServerEvent`, `OutboundPacket`) defined in `crates/ro-contracts`.

### 2.1 Framing & `PACKETVER` Rules
1. **Pinned Client Version & Packet Shuffling:**
   * The workspace targets `TARGET_PACKETVER = 20260219` exclusively. Legacy packet versions are omitted.
   * For `PACKETVER = 20260219`, map server incoming packet opcodes from `clif_shuffle.hpp` override `clif_packetdb.hpp`.
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
3. **Byte Offset Accuracy:**
   * In rAthena `parseable_packet(opcode, length, handler, pos0, pos1, ...)` definitions, trailing integer arguments specify exact byte offsets from byte 0 of the packet frame. Decoders must read fields at those exact offsets and skip intermediate padding bytes.
4. **Opcode & Length Framing:**
   * Every Ragnarok Online packet begins with a 2-byte little-endian `PacketOpcode` (`u16`).
   * `RathenaPacketLengthTable` implements `PacketLengthTable` (`crates/ro-contracts/src/packet.rs`), mapping each `(PacketOpcode, packetver)` to `PacketLength::Fixed(usize)` or `PacketLength::Dynamic`.
   * For `PacketLength::Dynamic`, bytes `2..4` contain a little-endian `u16` specifying the total frame byte length (including opcode and length field).
   * `PacketDecoder` implements `tokio_util::codec::Decoder`, reading from `BytesMut` and emitting `RawPacket { opcode, payload: Bytes }` once a complete frame has arrived.
5. **Graceful Unknown Opcode Handling:**
   * When `PacketLengthTable` knows the frame length of an unhandled opcode, `PacketDecoder` consumes the exact frame bytes and emits `RawPacket`, and `PacketIntentParser::parse_intent` returns `Ok(None)` so the session worker logs a `tracing::debug!` message and keeps the TCP connection open.
6. **32-bit Item IDs:**
   * Modern `PACKETVER` defines `MAX_ITEMID > UINT16_MAX`. Every item ID field on the wire and in contracts uses `u32`.

### 2.2 Coordinate & Direction Bit-Packing
Ragnarok Online packs map coordinates and directions into compact big-endian bitfields on the wire:
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
* Subsequent in-session packets (`CH_SELECT_CHAR` `0x0066`, `CZ_NOTIFY_ACTORINIT` `0x007d`, `CZ_REQUEST_TIME` `0x0360`, `CZ_REQUEST_MOVE` `0x035f`, `CZ_CHANGE_DIRECTION` `0x0361`, `CZ_REQUEST_ACT` `0x0437`, `CZ_REQNAME` `0x0368`) omit the sender's `AccountId` and `CharId` on the wire.
* `PacketIntentParser` extracts only the fields physically present in `RawPacket::payload`. Session identity (`SessionId`, `AccountId`, `CharId`) is tracked and attached by the binary session worker (`session.rs`) after handshake authentication.

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Traits Implemented:** `PacketLengthTable`, `PacketIntentParser`, and `EventEncoder`.
* **Isolation Invariant:** `crates/ro-packets` has zero dependency on `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, or `ro-scripting`. It never touches `TcpStream`, Tokio channels, SQL, or ECS state.
