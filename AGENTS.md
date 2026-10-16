# AGENTS.md: Project Metaling (Root Orchestrator & Architecture Spec)

## 1. Project Vision & Multi-Agent Operating Model
**Project Metaling** is a clean-room *Ragnarok Online* (RO) server workspace written in **Rust** using a custom **Entity-Component-System (ECS)** architecture.

To keep LLM context windows small and prevent cross-domain coupling, this Cargo workspace enforces a **Strict Subagent Isolation Protocol**:

### Role 1: The Main Agent (Root Orchestrator & Contract Arbiter)
If you are operating at the workspace root, you are the **Main Agent**.
* **Your Exclusive Write Scope:**
  * `./Cargo.toml` (Workspace manifest and shared dependencies)
  * `./AGENTS.md` and `./crates/<domain>/AGENTS.md` (Architecture, governance, and subagent domain specs)
  * `./crates/contracts/**` (Strictly pure cross-crate traits, DTOs, Event enums, error types, and ID newtypes)
  * `./bins/**` (`./bins/login-server/**`, `./bins/char-server/**`, `./bins/map-server/**`) (Executable binary entrypoints under the exclusive jurisdiction of the Main Agent)
  * `./tools/**` (`./tools/proxy/**`, auxiliary development and debugging tools under the exclusive jurisdiction of the Main Agent)
* **Strict Prohibition:** **YOU MUST NEVER traverse, read, or modify the implementation files inside `./crates/<domain>/src/**`** (except `./crates/contracts/**`).
* **Your Job:**
  1. Arbitrate whether a new cross-domain contract (trait, event struct, or DTO) is needed in `crates/contracts`.
  2. Update `crates/contracts` so downstream subagents have a compiling target interface.
  3. Delegate domain implementation tasks to the specific Subagent responsible for that library crate via `invoke_subagent`.
  4. Connect the completed library crates together inside `./bins/*` and `./tools/*`.

#### Mandatory Operating Rules for the Main Agent:
1. **Strict Interface Purity in `crates/contracts`:**
   `crates/contracts` contains zero business logic, zero concrete codecs, zero wire framing loops, and zero heavy transport dependencies. Only abstract data structures, enums, newtypes, and traits belong in this crate. Concrete framing algorithms and serialization logic belong in domain crates such as `crates/packets`.
2. **Mandatory Subagent Delegation:**
   The Main Agent must never implement domain logic, including packet framing, packet parsing, packet serialization, coordinate bit-packing, database repositories, combat formulas, or ECS systems. When new domain functionality is required, the Main Agent defines the compiling interface in `crates/contracts` and immediately invokes the corresponding domain subagent using `invoke_subagent`.
3. **Binary Restraint and Exclusive Jurisdiction over `bins/*` and `tools/*`:**
   All binaries under `./bins/*` and auxiliary tools under `./tools/*` remain under the exclusive write scope of the Main Agent. Binary entrypoints are restricted to bootstrap configuration, dependency injection, inter-server HTTP routing, TCP session supervision, proxying, and high-level actor coordination that calls domain crate functions. Inline gameplay state machines, bespoke protocol decoders, and hardcoded entity repositories inside `bins/*` are prohibited.

### Role 2: Domain Subagents (Crate-Scoped Specialists)
Each library crate under `./crates/<domain>/` has its own local `AGENTS.md` and is served by a dedicated Subagent.
* **Allowed Read Scope:**
  1. Its own crate directory (`./crates/<domain>/**`).
  2. The shared contract crate (`./crates/contracts/**`, including `./crates/contracts/src/**`). Subagents are encouraged to read contract definitions directly from disk.
  3. Any file inside `./reference/**` (`rathena`, `Hercules`, `rust-ro`, `rathena-script-lang-interpreter`) needed to extract domain specifications.
* **Direct Inspection of Contracts (No `cargo doc`):** Subagents must read the Rust files in `./crates/contracts/src/` directly via file viewing tools. Subagents must avoid invoking `cargo doc` or background documentation builds to inspect contracts, as this introduces unnecessary compilation overhead.
* **Allowed Write Scope:** **ONLY** `./crates/<domain>/src/**` and `./crates/<domain>/Cargo.toml`.
* **Strict Prohibition:** A Subagent **MUST NEVER** write to `./crates/contracts/**`, `./bins/**`, `./tools/**`, or another domain crate (`./crates/<other_domain>/**`). Subagents also must never read implementation files inside other domain crates (`./crates/<other_domain>/src/**`). If a Subagent needs a missing field, event, or trait method in `crates/contracts`, it must **STOP** and request a Contract Amendment from the Main Agent.

---

## 2. Workspace Layout & Delegation Decision Matrix

```text
project-metaling/
├── AGENTS.md                 # [Main Agent] Master architecture & orchestration rules
├── Cargo.toml                # [Main Agent] Workspace manifest
├── reference/                # [Read-Only for All Agents] Cloned C/C++/Rust reference repos
├── crates/
│   ├── contracts/            # [Main Agent] Shared traits, IDs, ClientIntent, OutboundPacket, DTOs
│   ├── packets/              # [Subagent] Binary wire protocol, framing, PACKETVER structs, bit-packing
│   ├── engine/               # [Subagent] ECS World, 8x8 SpatialGrid, Map tick loop, AoI, movement, entities
│   ├── db/                   # [Subagent] Static YAML database loaders (item_db, mob_db, skill_db) & .gat map cache
│   ├── mechanics/            # [Subagent] Pure stateless math functions (stats, damage Context, status rules)
│   ├── persistence/          # [Subagent] PostgreSQL (sqlx) & stub repositories for accounts/characters
│   └── scripting/            # [Subagent] Adapter around rathena-script-lang-interpreter for NPC/item scripts
├── bins/
│   ├── login-server/         # [Main Agent] TCP :6900 & HTTP :6901 binary entrypoint
│   ├── char-server/          # [Main Agent] TCP :6121 & HTTP :6122 binary entrypoint
│   └── map-server/           # [Main Agent] TCP :5121 & HTTP :5122 binary entrypoint (hosts `crates/engine`)
└── tools/
    └── proxy/                # [Main Agent] RO packet logging & forwarding proxy (:6900, :6121, :5121)
```

### 2.1 Behavior Delegation Matrix
When implementing a feature or fixing a bug, the Main Agent uses this matrix to determine where each behavior belongs and how crates interact through `crates/contracts`:

| Behavior / Responsibility | Owner Scope | Contract Interface (`crates/contracts`) | Upstream Caller / Consumer |
| :--- | :--- | :--- | :--- |
| **Cross-crate IDs, DTOs, Intents, Events, & Traits** | `crates/contracts` (Main Agent) | `ids.rs`, `packet.rs`, `map.rs`, `http.rs`, repository & engine traits | All domain crates and `bins/*` |
| **TCP framing, opcode length table, byte parsing, event encoding, `WBUFPOS`/`WBUFPOS2` bit-packing** | `crates/packets` (Subagent) | `PacketLengthTable`, `PacketIntentParser`, `EventEncoder`, `RawPacket` | `bins/{login,char,map}-server/src/session.rs` |
| **`.gat`/`.rsw` walkability grids, `item_db`/`mob_db`/`skill_db` YAML loading, spawn/warp file parsing** | `crates/db` (Subagent) | `MapWalkability`, static blueprint DTOs, and database provider traits | Initialized by `bins/map-server`, queried by `crates/engine` |
| **Derived stats (`ATK`, `DEF`, `ASPD`, `HIT`, `FLEE`), elemental/size/race modifiers, damage formulas** | `crates/mechanics` (Subagent) | Pure stateless calculator traits and `DamageContext` / stat DTOs | Invoked by `crates/engine` during combat and status ticks |
| **Account authentication, character slot CRUD, in-memory stub repo (Phases 0 to 3), PostgreSQL `sqlx` repo (Phase 4+)** | `crates/persistence` (Subagent) | `AccountRepository`, `CharacterRepository`, `CharacterSummary` | Injected into `LoginService`, `CharService`, and `MapService` |
| **NPC conversation execution (`mes`, `next`, `select`, `warp`) & item script evaluation** | `crates/scripting` (Subagent) | Script VM runner traits and script suspension/resumption events | Invoked by `crates/engine` during entity interaction phases |
| **ECS `World`, `8x8` `SpatialGrid`, A* pathfinding over `MapWalkability`, AoI calculation, entity lifecycle, deterministic tick loop** | `crates/engine` (Subagent) | Consumes `ClientIntent` (with session/character context), emits `ScopedServerEvent` | Hosted and ticked by `bins/map-server` (`MapService`) |
| **TCP listener loops, raw 4-byte `AccountId` echo, `session.rs` channel bridge, Axum HTTP routers/clients, service dependency injection** | `bins/*` (Main Agent) | Consumes `crates/contracts` traits and wires concrete domain types together | Executed directly as server binaries |

### 2.2 Judgement Rules: What Belongs in `bins/*` vs. Domain Crates
1. **No Domain State in `bins/*/src/service.rs`:**
   * `LoginService` (`bins/login-server`) and `CharService` (`bins/char-server`) must delegate account lookups, credential checks, and character list/creation/deletion storage to `crates/persistence` via `AccountRepository` and `CharacterRepository`. Even during Phases 0 to 3, stub account and character records belong in `crates/persistence::InMemoryStubRepo` rather than hardcoded structs inside `bins/*`.
   * `MapService` (`bins/map-server`) must delegate map state, default spawn coordinates, entity tracking, server tick generation, movement, and AoI event emission to `crates/engine`. `MapService` exists only to run the tick timer, feed session-authenticated intents into `crates/engine`, coordinate map transfers, and route emitted `ScopedServerEvent` payloads to client sessions.
2. **Protocol vs. Session Context:**
   * `crates/packets` owns wire-level framing and opcode-to-struct decoding/encoding.
   * `bins/*/src/session.rs` owns the live `TcpStream`, the raw 4-byte `AccountId` handshake echo required by `CH_ENTER` and `CZ_ENTER`, and per-connection session state (`SessionId`, authenticated `AccountId`, and `CharId`).
3. **Multi-Subagent Delegation Sequencing:**
   When a roadmap deliverable spans multiple crates, the Main Agent executes work in four stages:
   * **Stage 1 (Contracts First):** Update `crates/contracts` with all required traits, DTOs, and enum variants. Run `cargo check -p ro-contracts` to verify compilation.
   * **Stage 2 (Leaf Providers in Parallel):** Invoke subagents for leaf crates (`crates/packets`, `crates/db`, `crates/mechanics`, `crates/persistence`, `crates/scripting`) concurrently via `invoke_subagent`. These crates depend only on `crates/contracts`.
   * **Stage 3 (Engine Integration):** Invoke the `crates/engine` subagent to implement ECS systems consuming the contract traits. (If all contract traits were already finalized in Stage 1, Stage 3 may run concurrently with Stage 2.)
   * **Stage 4 (Binary Wiring & Verification):** Instantiate the concrete providers from `db`, `mechanics`, `persistence`, `scripting`, `packets`, and `engine` inside `bins/*`, and run workspace integration tests.

---

## 3. Reference Repositories Routing Table (`./reference/`)
All agents may freely inspect `./reference/` to extract protocol specs, YAML schemas, and Aegis mechanics. **Never copy C/C++ pointer control flow or `rust-ro`'s non-ECS service layer.**

| Domain Crate | Primary Reference Paths | What to Extract |
| :--- | :--- | :--- |
| **`crates/packets`** | • `reference/rathena/src/config/packets.hpp`<br>• `reference/rathena/src/map/packets_struct.hpp`<br>• `reference/rathena/src/map/packets.hpp`<br>• `reference/rathena/src/map/clif_packetdb.hpp`<br>• `reference/rathena/src/map/clif_shuffle.hpp`<br>• `reference/rathena/src/login/loginclif.cpp`<br>• `reference/rathena/src/char/char_clif.cpp`<br>• `reference/rust-ro/lib/packets/` | Latest `PACKETVER` struct layouts from `rathena`. Binary reader/writer patterns and 3-byte (`WBUFPOS`) / 6-byte (`WBUFPOS2`) coordinate bit-packing from `rust-ro`. Inspect `clif_shuffle.hpp` for shuffled incoming map opcodes overriding `clif_packetdb.hpp`. |
| **`crates/engine`** | • `reference/Hercules/src/map/*.h`<br>• `reference/rathena/src/map/map.cpp`<br>• `reference/rathena/src/map/pc.cpp`<br>• `reference/rathena/src/map/unit.cpp` | High-level state machine lifecycles (login spawn handshake, walking ticks, sitting, portal warp triggers, AoI broadcast ranges). Re-implement inside ECS. |
| **`crates/db`** | • `reference/rathena/db/re/` (and `db/import/`)<br>• `reference/rathena/npc/re/warps/` & `mobs/`<br>• `reference/rust-ro/tools/map-cache/` | `serde_yaml` schemas for items, mobs, skills, and job tables; `.gat`/`.rsw` map cell walkability parsing; monster/warp spawn definitions. |
| **`crates/mechanics`** | • `reference/rathena/src/map/battle.cpp`<br>• `reference/rathena/src/map/status.cpp`<br>• `reference/rathena/src/map/skills/<job>/*.cpp`<br>• `reference/rust-ro/tools/stat-calc/` | Pure mathematical formulas, ASPD/stat derivations, elemental/size/race tables, and `SkillImpl` ratios. |
| **`crates/persistence`** | • `reference/rathena/sql-files/main.sql`<br>• `reference/rust-ro/server/src/repository/` | Account, character, and inventory relational table designs adapted for modern PostgreSQL (`sqlx`). |
| **`crates/scripting`** | • `reference/rathena-script-lang-interpreter/src/` | VM hooks and native function bindings to bridge NPC scripts with ECS events. |

---

## 4. Target `PACKETVER` Policy (Pinned Version: 20260219)
* **Exclusive Target Version:** The workspace pins `PACKETVER = 20260219` as the single client version. The server omits backwards compatibility for older packet versions.
* **Packet Shuffling Precedence (`clif_shuffle.hpp`):** For `PACKETVER = 20260219`, map server incoming packet opcodes are defined in `reference/rathena/src/map/clif_shuffle.hpp`. Definitions in `clif_shuffle.hpp` take precedence over `clif_packetdb.hpp`.
* **Canonical Core Opcodes for `20260219`:**
  * `CZ_ENTER`: `0x0436` (Fixed 23 bytes; account id at byte 2, char id at byte 6, auth code at byte 10, client tick at byte 14, 4 bytes padding at 18..22, sex at byte 22)
  * `CZ_NOTIFY_ACTORINIT`: `0x007d` (Fixed 2 bytes)
  * `CZ_REQUEST_TIME`: `0x0360` (Fixed 6 bytes; client tick at byte 2)
  * `CZ_REQUEST_MOVE`: `0x035f` (Fixed 5 bytes; 3-byte packed coords at byte 2)
  * `CZ_CHANGE_DIRECTION`: `0x0361` (Fixed 5 bytes; head dir at byte 2, body dir at byte 4)
  * `CZ_REQUEST_ACT`: `0x0437` (Fixed 7 bytes; target id at byte 2, action type at byte 6)
  * `CZ_REQNAME`: `0x0368` (Fixed 6 bytes; entity id at byte 2)
  * `AC_ACCEPT_LOGIN`: `0x0ac4` (Dynamic)
  * `HC_NOTIFY_ZONESVR`: `0x0ac5` (Fixed 156 bytes)
* **Macro Offset Parsing:** In `parseable_packet(opcode, length, handler, pos0, pos1, ...)`, the trailing integer arguments define exact byte offsets from byte 0 of the packet frame. Decoders must follow these positions to read fields accurately.
* **Extraction Tooling (No Standalone C++ Builds):** Do not attempt to compile standalone C++ binaries against rAthena headers. The headers transitively require MySQL client libraries and internal subsystem headers. Use the C preprocessor directly (`clang -E -DPACKETVER=20260219 ...`) or ripgrep pattern searches over macro definitions.
* **32-bit Item IDs (`u32`):** Modern `PACKETVER` defines `MAX_ITEMID > UINT16_MAX`. All item ID fields in packets and contracts must use `u32`.
* **Graceful Unknown Packet Logging:** If the server receives an opcode not yet required for the current phase, log `tracing::debug!("Unhandled packet opcode={:#06x} len={}", opcode, bytes.len())`, consume the frame bytes using the packet length table, and continue the session loop.

---

## 5. Incremental Implementation Roadmap (Phases 0 to 4)

### Phase 0: Three-Server Network Handshake & Map Entry (Stub State)
* **Goal:** Connect an official client matching `PACKETVER`, log in with a hardcoded stub account/character, select the Character Server, select the character, and load onto a starter map (`prontera`).
* **Deliverables:**
  1. `login-server` (`:6900`): Handle `CA_LOGIN`, issue session token `(AccountId, LoginId1, LoginId2)`, send `AC_ACCEPT_LOGIN` with `char-server` IP/port.
  2. `char-server` (`:6121`): Handle `CH_ENTER`, echo raw 4-byte `AccountId`, send `HC_ACCEPT_ENTER` (and character page packets for modern `PACKETVER`) with 1 hardcoded stub character, handle `CH_SELECT_CHAR`, reply with `HC_NOTIFY_ZONESVR` (`prontera.gat`, `map-server` IP/port).
  3. `map-server` (`:5121`): Handle `CZ_ENTER`, echo 4-byte `AccountId` + `ZC_ACCEPT_ENTER` (spawn coords + server tick), handle `CZ_NOTIFY_ACTORINIT` (`0x007D` "Map Loaded"), respond with initial actor state (`ZC_NOTIFY_NEWENTRY` / `ZC_PAR_CHANGE`), and reply to `CZ_REQUEST_TIME` (`0x0360`) with `ZC_NOTIFY_TIME` so the client does not time out.
  4. All other incoming packets log their opcode + payload length and are safely ignored.

### Phase 1: Map Movement & Sitting
* **Goal:** Walk around `prontera` with smooth pathing and toggle sitting/standing.
* **Deliverables:**
  1. `crates/db`: Load `.gat` walkability grids for maps.
  2. `crates/engine`: Implement A* pathfinding over `.gat` cells, `WalkingPath` sparse component, and `MapPosition` updates in the 8x8 `SpatialGrid`.
  3. Handle `CZ_REQUEST_MOVE` -> emit `ZC_NOTIFY_PLAYERMOVE` (6-byte bit-packed coordinates `WBUFPOS2`).
  4. Handle `CZ_CHANGE_DIRECTION` (head/body direction) and `CZ_REQUEST_ACT` (`action = 2` sit, `action = 3` stand) -> broadcast `ZC_NOTIFY_ACT` to AoI.

### Phase 2: Static Entities (Monsters & Map Portals)
* **Goal:** See monsters (stationary) and warp portal visual effects spawned on the map.
* **Deliverables:**
  1. `crates/db`: Parse `mob_db.yml` and map spawn/warp definitions.
  2. `crates/engine`: Spawn Monster ECS entities and Portal (`GroundSkillUnit` / warp NPC) entities into the `SpatialGrid`.
  3. Broadcast `ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY` when a player enters AoI or finishes `CZ_NOTIFY_ACTORINIT`, and handle `CZ_REQNAME` (`0x0368`) -> `ZC_ACK_REQNAME` so hovering over a monster displays its name.

### Phase 3: Map Switching & Portal Collision
* **Goal:** Walk into a warp portal cell and seamlessly transition to the destination map.
* **Deliverables:**
  1. `crates/engine`: Detect when a player's `MapPosition` steps within the trigger radius of a Warp Portal entity.
  2. Despawn player from the current Map `World`, transfer entity state to the destination Map `World`, and emit `ZC_NPCACK_MAPMOVE` (`0x0091`) with the new `.gat` name and `(X, Y)` coordinates.
  3. Re-run the `CZ_NOTIFY_ACTORINIT` (`0x007D`) map-load synchronization cycle on arrival.

### Phase 4: PostgreSQL Persistence & Character Creation
* **Goal:** Replace hardcoded stubs with real PostgreSQL persistence using `sqlx`.
* **Deliverables:**
  1. `crates/persistence`: Implement PostgreSQL migrations and queries for `accounts` and `characters` (persisting `account_id`, `char_id`, `name`, `slot`, `sex`, `hair_style`, `hair_color`, `last_map`, `last_x`, `last_y`).
  2. `char-server`: Implement `CH_MAKE_CHAR` (`0x0067` / `0x0A39`) and `CH_DELETE_CHAR`, persisting new characters to PostgreSQL.
  3. `map-server`: Load character spawn location/appearance from `crates/persistence` on login and flush updated `(last_map, last_x, last_y)` on movement/warp/disconnect.

---

## 6. Network Topology & Communication Boundaries

All server binaries execute on the Tokio asynchronous runtime. Network responsibilities partition into client-facing TCP streaming and internal inter-server HTTP communication.

### 6.1 Client Boundaries (TCP Binary Protocol)
Clients connect to designated TCP ports using the Ragnarok Online binary packet protocol.
* **`login-server` (TCP :6900):**
  Accepts transient connections. Decodes `CA_LOGIN`, issues a `SessionTicket`, returns `AC_ACCEPT_LOGIN` containing the list of available character servers, and drops the TCP connection.
* **`char-server` (TCP :6121):**
  Accepts transient connections. Decodes `CH_ENTER`, verifies the `SessionTicket`, echoes the 4-byte `AccountId`, returns `HC_ACCEPT_ENTER` containing character slots, decodes `CH_SELECT_CHAR`, returns `HC_NOTIFY_ZONESVR` pointing to the target map server, and drops the TCP connection.
* **`map-server` (TCP :5121):**
  Accepts persistent gameplay connections. Decodes `CZ_ENTER`, echoes `AccountId`, returns `ZC_ACCEPT_ENTER`. Decodes `CZ_NOTIFY_ACTORINIT`, synchronizes map state, and maintains the tick loop exchanging `CZ_REQUEST_TIME` and `ZC_NOTIFY_TIME`. Decodes player movement and action intents, then broadcasts outbound packets across Area of Interest (AoI) subscribers.

### 6.2 Inter-Server Boundaries (Internal Axum HTTP APIs)
Coordination between server binaries runs over Axum HTTP services using JSON payloads defined in `crates/contracts/src/http.rs`.
* **Login Server Internal API (:6901):**
  * `POST /api/v1/sessions/validate`: Validates an active `SessionTicket` presented to `char-server`.
  * `POST /api/v1/servers/register`: Registers a `char-server` instance and updates its capacity.
  * `POST /api/v1/servers/heartbeat`: Receives periodic health and user count telemetry.
  * `GET /api/v1/health`: Health probe endpoint.
* **Char Server Internal API (:6122):**
  * `POST /api/v1/maps/register`: Registers a `map-server` instance and its hosted maps.
  * `POST /api/v1/transfers/prepare`: Coordinates character transfer reservations prior to client redirection.
  * `GET /api/v1/health`: Health probe endpoint.
* **Map Server Internal API (:5122):**
  * `POST /api/v1/transfers/accept`: Confirms an incoming player transfer ticket.
  * `GET /api/v1/health`: Health probe reporting map instances, player count, and tick rate.

### 6.3 Packet Framing Interface (Referencing `rust-ro`)
The packet interface builds upon the framing mechanics in `reference/rust-ro/lib/packets/` and `reference/rathena/src/map/clif_packetdb.hpp`.
* **Opcode Header:** Every packet begins with a 2-byte little-endian `PacketOpcode`.
* **Length Evaluation:** The `PacketLengthTable` trait evaluates the opcode into either `PacketLength::Fixed(usize)` or `PacketLength::Dynamic`. Dynamic packets contain a 2-byte little-endian length at bytes 2..4 specifying total packet size.
* **Tokio Decoder:** `PacketDecoder` implements `tokio_util::codec::Decoder`, producing `RawPacket` items containing the opcode and frozen `bytes::Bytes` payload.
* **Outbound Broadcasts:** `OutboundPacket` encapsulates zero-copy `bytes::Bytes` tagged with `OutboundTarget`, enabling multicast across spatial entities without duplicate memory allocation.

### 6.4 Unified Protocol-to-Logic Channel Bridge
All three server binaries (`login-server`, `char-server`, `map-server`) enforce a uniform decoupling pattern between network protocol workers (`session.rs`) and application logic (`service.rs`) using Tokio asynchronous channels (`tokio::sync::mpsc`). Channels are scoped to the unit of sequential consistency for each server type:

```text
  [login-server & char-server: Ephemeral Per-Session Logic Actor]
  +-----------------------+    mpsc (SessionIntent)    +----------------------------+
  | Protocol Worker       | -------------------------> | Ephemeral Session Actor    |
  | (session.rs)          | <------------------------- | (LoginSessionActor /       |
  | Split Reader / Writer |    try_send (Bytes/Event)  |  CharSessionActor)         |
  +-----------------------+                            +----------------------------+

  [map-server: Long-Lived Per-World Logic Actor]
  +-----------------------+    mpsc (SessionIntent)    +----------------------------+
  | Protocol Worker       | -------------------------> | Long-Lived MapWorldActor   |
  | (session.rs)          | <------------------------- | (ro-engine World +         |
  | Split Reader / Writer |    try_send (Frozen Bytes) |  Deterministic Tick Loop)  |
  +-----------------------+                            +----------------------------+
```

1. **Protocol Worker (`session.rs`):**
   * Owns the client `TcpStream`, splitting it into an **Inbound Reader** (`OwnedReadHalf`) and an **Outbound Writer** (`OwnedWriteHalf`) so inbound packet decoding and outbound socket writes never block or deadlock each other.
   * Tracks per-connection session state (`SessionId`, and the authenticated `AccountId` / `CharId` established during `CH_ENTER` or `CZ_ENTER`).
   * Utilizes `PacketDecoder` and `PacketIntentParser` to produce pure wire-level `ClientIntent` instances from raw incoming bytes, then wraps them in a `SessionIntent` envelope containing `(SessionId, Option<AccountId>, Option<CharId>, ClientIntent)`.
   * Enforces sequential per-session handshake state: post-handshake packets (`SelectCharacter`, `NotifyActorInit`, `RequestTime`, `RequestMove`, `ChangeDirection`, `RequestAction`, `RequestEntityName`) are rejected until the session has completed its entry handshake (`EnterCharServer` or `EnterMap`).
   * For wire protocol handshakes requiring a raw 4-byte `AccountId` echo (`char-server`, `map-server`), queues the 4-byte payload to the Outbound Writer immediately upon receiving `ClientIntent::EnterCharServer` or `ClientIntent::EnterMap`.

2. **Logic Layer & Actor Lifetimes Across Servers:**
   * **Non-Blocking Outbound Dispatch:** Logic actors must never `.await` on a per-client outbound channel inside a shared loop, as a single slow or stalled TCP client would block other players. Outbound delivery uses non-blocking `try_send` to the session's Outbound Writer queue (disconnecting overflowed sessions).
   * **`login-server` (Ephemeral `LoginSessionActor` per Client Session):**
     Client connections to `login-server` are transient handshakes. Each accepted connection spawns an ephemeral `LoginSessionActor` paired with `session.rs` via dedicated `mpsc` channels and holding shared `Arc` handles to `AccountRepository` and the character server registry. It processes `ClientIntent::Login` sequentially for that client and exits when the session closes.
   * **`char-server` (Ephemeral `CharSessionActor` per Client Session):**
     Client connections to `char-server` are transient character selection sessions. Each accepted connection spawns an ephemeral `CharSessionActor` paired with `session.rs` via dedicated `mpsc` channels and holding shared `Arc` handles to `CharacterRepository` and the map server registry. Because the actor is dedicated to one client, `EnterCharServer` completes and binds the session's `AccountId` before `SelectCharacter`, `MakeCharacter`, or `DeleteCharacter` is processed, without blocking unrelated accounts during database or HTTP queries.
   * **`map-server` (Long-Lived `MapWorldActor` per Map Instance):**
     Map instances (`prontera.gat`, etc.) are persistent simulation worlds hosted by `MapService`. Each active map runs a long-lived `MapWorldActor` owning a `ro-engine` `World`, a periodic tick timer, and a map-scoped `mpsc` inbox. Player sessions bind to their current map's `MapWorldActor` upon `EnterMap` and forward `SessionIntent` messages in FIFO order. At each tick, the `MapWorldActor` drains queued intents, executes the deterministic ECS phases, encodes each emitted `ScopedServerEvent` once into frozen `bytes::Bytes`, and fans out the `Bytes` via non-blocking `try_send` to the outbound writer channels of target sessions in the Area of Interest. When a player steps into a warp portal, their session re-binds to the destination map's `MapWorldActor`.
