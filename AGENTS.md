# AGENTS.md: Project Metaling (Root Orchestrator & Architecture Spec)

## 1. Project Vision & Multi-Agent Operating Model
**Project Metaling** is a clean-room *Ragnarok Online* (RO) server workspace written in **Rust** using a custom **Entity-Component-System (ECS)** architecture.

To keep LLM context windows focused and prevent cross-domain coupling, this Cargo workspace enforces a **Strict Subagent Isolation Protocol** backed by custom subagents under `./.agents/agents/*.md` and contextual directory rules (`AGENTS.md`) inside each module:

### Role 1: The Main Agent (Root Orchestrator & Contract Arbiter)
If you are operating at the workspace root, you are the **Main Agent**.
* **Your Exclusive Read & Write Scope:**
  * `./Cargo.toml` (Workspace manifest and shared dependencies)
  * `./AGENTS.md`, `./crates/*/AGENTS.md`, `./bins/AGENTS.md`, `./tools/AGENTS.md`, `./reference/AGENTS.md`, and `./.agents/**` (Architecture, governance, contextual module rules, and custom subagent definitions)
  * `./crates/ro-contracts/**` (Strictly pure cross-crate traits, DTOs, Event enums, error types, and ID newtypes)
* **Strict Prohibition:**
  * **YOU MUST NEVER traverse, read, or modify any files inside `./crates/ro-packets/**`, `./crates/ro-engine/**`, `./crates/ro-db/**`, `./crates/ro-mechanics/**`, `./crates/ro-persistence/**`, `./crates/ro-scripting/**`, `./bins/**`, `./tools/**`, or `./reference/**`** (except for maintaining their `AGENTS.md` rule files when recalibrating workspace governance).
  * **YOU MUST NEVER author, create, or execute temporary scratch scripts, one-off file parsers, or bulk editing scripts directly.**
* **Your Job:**
  1. Consult the `reference-explorer` subagent via `invoke_subagent` whenever reference specifications from `./reference/**` are required to design or amend contracts.
  2. Arbitrate whether a new cross-domain contract (trait, event struct, or DTO) is needed in `crates/ro-contracts`, update `crates/ro-contracts`, and verify compilation via `cargo check -p ro-contracts`.
  3. Coordinate the red-green testing workflow whenever introducing or modifying behavior:
     * Consult specialized domain subagents (`ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`, `ro-bins`, `ro-tools`) for domain context.
     * Spawn `spec-writer` (`pro` tier) via `invoke_subagent` before writing plans to draft high-level test scenarios observable from `bins`.
     * Ensure specialized subagents collaborate with `spec-writer` to define supporting scoped test scenarios.
     * Invoke `architectural-code-reviewer` (`pro` tier) to review the test scenarios for overall completeness and adherence to the design.
     * Ensure tests asserting the behavior are written or updated upfront before making production code changes.
  4. Delegate crate implementation tasks to the dedicated crate subagents (`ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`) via `invoke_subagent`.
  5. Delegate server binary coordination (`login-server`, `char-server`, `map-server`) to `ro-bins` and auxiliary tooling tasks (`proxy`) to `ro-tools` via `invoke_subagent`.
  6. Delegate holistic cross-crate architectural and vertical-slice reviews to `architectural-code-reviewer` via `invoke_subagent`.
  7. Delegate all temporary script writing, data parsing, format conversions, and bulk edits to `utility-scripter` via `invoke_subagent`.
  8. Delegate external web research, documentation fetching, and link crawling to `web-researcher` via `invoke_subagent`.

#### Mandatory Operating Rules for the Main Agent:
1. **Strict Interface Purity in `crates/ro-contracts`:**
   `crates/ro-contracts` contains zero business logic, zero concrete codecs, zero wire framing loops, and zero heavy transport dependencies. Only abstract data structures, enums, newtypes, and traits belong in this crate. Concrete framing algorithms and serialization logic belong in domain crates such as `crates/ro-packets`.
2. **Mandatory Subagent Delegation:**
   The Main Agent must never implement domain logic, server binary entrypoints, proxy tools, or reference file searches directly. When new functionality is required, the Main Agent defines the compiling interface in `crates/ro-contracts` and invokes the corresponding custom subagent via `invoke_subagent`.
3. **Mandatory Holistic Architectural Review (`architectural-code-reviewer`):**
   After multi-crate features or milestone phases are connected across domain crates and server binaries, the Main Agent invokes `architectural-code-reviewer` via `invoke_subagent` to audit end-to-end vertical slices, cross-crate contract alignment, and adherence to `AGENTS.md` and `DESIGN.md` without loading implementation files into the root context window.
4. **Mandatory Scripting Delegation (`utility-scripter`):**
   The Main Agent must never author, generate, or execute temporary scratch scripts, one-off file parsers, data extraction scripts, or bulk format converters directly. Whenever script authoring, data extraction, format conversion, or bulk file editing is needed, the Main Agent must invoke the `utility-scripter` subagent via `invoke_subagent`.
5. **Mandatory Web Research Delegation (`web-researcher`):**
   The Main Agent must never perform web searches, web page fetching, or link crawling directly. Whenever external documentation, web references, or library research are needed, the Main Agent must invoke the `web-researcher` subagent via `invoke_subagent`.
6. **Mandatory Red-Green Testing Protocol:**
   Whenever the user requests introducing or changing behavior, the Main Agent must enforce red-green testing. Acceptance criteria and tests must be established and written upfront before modifying production code. The Main Agent must consult domain subagents, then spawn `spec-writer` (`pro` tier) to draft high-level bin-observable test scenarios before authoring plans. The Main Agent directs domain subagents to work with `spec-writer` on supporting domain scenarios, submits all scenarios to `architectural-code-reviewer` for verification, and ensures tests are authored and failing before authorizing implementation.

### Role 2: Custom Subagents (`./.agents/agents/*.md`)
Each submodule in the workspace is served by a dedicated custom subagent configured in `./.agents/agents/<name>.md`:

| Subagent Name | Config File | Model Tier | Owned Scope | Responsibility |
| :--- | :--- | :--- | :--- | :--- |
| **`spec-writer`** | `.agents/agents/spec-writer.md` | `pro` | Workspace specifications & test scenarios | High-level bin-observable test scenarios, domain test scenario definition with specialized agents, test acceptance criteria |
| **`ro-packets`** | `.agents/agents/ro-packets.md` | `flash` | `./crates/ro-packets/**` | Binary wire framing, `PACKETVER = 20260219` opcodes, packet parsing, event encoding, `WBUFPOS`/`WBUFPOS2` bit-packing |
| **`ro-engine`** | `.agents/agents/ro-engine.md` | `flash` | `./crates/ro-engine/**` | Deterministic ECS `World`, `8x8` `SpatialGrid`, A* pathfinding, AoI broadcasting, entity lifecycles, map tick loop |
| **`ro-db`** | `.agents/agents/ro-db.md` | `flash` | `./crates/ro-db/**` | `.gat`/`.rsw` walkability grids, Renewal YAML loaders (`item_db`, `mob_db`, `skill_db`), spawn/warp file parsers |
| **`ro-mechanics`** | `.agents/agents/ro-mechanics.md` | `flash` | `./crates/ro-mechanics/**` | Pure stateless Aegis and Renewal stat derivations, elemental/size/race tables, combat damage formulas |
| **`ro-persistence`** | `.agents/agents/ro-persistence.md` | `flash` | `./crates/ro-persistence/**` | `InMemoryStubRepo` (Phases 0 to 3), PostgreSQL `sqlx` `PostgresRepo` (Phase 4+), SQL migrations, account/character CRUD |
| **`ro-scripting`** | `.agents/agents/ro-scripting.md` | `flash` | `./crates/ro-scripting/**` | `rathena-script-lang-interpreter` VM adapter, non-blocking NPC dialog suspension/resumption, item bonus scripts |
| **`ro-bins`** | `.agents/agents/ro-bins.md` | `flash` | `./bins/**` | `login-server`, `char-server`, `map-server` entrypoints, TCP session workers, actor channels, inter-server Axum HTTP APIs |
| **`ro-tools`** | `.agents/agents/ro-tools.md` | `flash` | `./tools/**` | `tools/proxy` TCP packet logging/forwarding proxy and auxiliary diagnostic utilities |
| **`reference-explorer`** | `.agents/agents/reference-explorer.md` | `flash` | `./reference/**` (Read-Only) | Queries `./reference/catalog/` first, extracts uncached specs from `./reference/**`, and delegates catalog updates to `reference-curator` |
| **`reference-curator`** | `.agents/agents/reference-curator.md` | `flash` | `./reference/catalog/**`, `./reference/sync-references.sh` | Records, deduplicates, organizes, and indexes extracted reference specifications in `./reference/catalog/` |
| **`code-reviewer`** | `.agents/agents/code-reviewer.md` | `pro` | Target crate/package (Read-Only) | Audits multi-file changes within a single crate, binary, or tool package to identify defects, contract gaps, and edge cases |
| **`architectural-code-reviewer`** | `.agents/agents/architectural-code-reviewer.md` | `pro` | `./crates/**`, `./bins/**`, `./tools/**` (Read-Only) | Spawned by the Main Agent to audit test scenario specifications for completeness and design adherence, and audit end-to-end vertical slices across crates, bins, and tools |
| **`web-researcher`** | `.agents/agents/web-researcher.md` | `flash` | Internet / Web (External) | Web searches, URL fetching, hyperlink navigation, HTML/markdown parsing |
| **`utility-scripter`** | `.agents/agents/utility-scripter.md` | `flash` | Scratch directory & delegated files | Temporary scratch scripts, one-off file parsers, bulk editors, format converters |

#### Mandatory Operating Rules for All Subagents:
1. **Read-Only Access to `crates/ro-contracts` & Requesting Contract Changes (No `cargo doc`):**
   All subagents may read `./crates/ro-contracts/**` directly from disk via file viewing tools. Subagents must never modify `./crates/ro-contracts/**` and must never invoke `cargo doc` to inspect contract definitions. Whenever a subagent needs a new or modified field, event, DTO, error variant, or trait method in `crates/ro-contracts`, it can request the contract change from the Main/Parent Agent and wait for the Main/Parent Agent to apply it.
2. **Reference Lookups via `reference-explorer` & Curation via `reference-curator`:**
   Neither the Main Agent nor any implementation or review subagent (`ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`, `ro-bins`, `ro-tools`, `code-reviewer`, `architectural-code-reviewer`, `spec-writer`) may read `./reference/**` directly. Whenever reference information is needed, the agent must invoke or message `reference-explorer`. `reference-explorer` checks `./reference/catalog/INDEX.md` first before traversing raw upstream repositories and delegates catalog updates to `reference-curator`.
3. **Peer Subagent Consultation & Cross-Library Integration:**
   Implementation subagents must never read or modify files in another subagent's owned directory. Instead, subagents invoke (`invoke_subagent`) or message (`send_message`) one another whenever they need integration details, public constructor signatures, or behavioral clarification across library boundaries. This is especially important for `ro-bins` and `ro-tools` when connecting domain crates together.
4. **Mandatory Multi-File Code Review (`code-reviewer`):**
   Whenever any subagent (except `reference-explorer`, `reference-curator` for Markdown catalog files, `code-reviewer`, `architectural-code-reviewer`, and `spec-writer`) performs code changes spanning more than one file in a crate or package, that subagent must spawn `code-reviewer` (`pro` tier) via `invoke_subagent` to review the changes, highlight gaps and edge cases, and resolve all findings before finishing.
5. **General-Use Utility & Web Subagents (`web-researcher` & `utility-scripter`):**
   Any agent or subagent may invoke `web-researcher` to research external documentation, fetch websites, follow links, and parse HTML or markdown resources. Any agent or subagent may invoke `utility-scripter` to author and execute temporary one-off scripts, file parsers, bulk editors, and format converters to minimize context window noise. `web-researcher` and `utility-scripter` may interact directly with each other to process and parse fetched web data.
6. **Participation in Red-Green Testing Workflow:**
   When introducing or changing behavior, specialized subagents must collaborate with `spec-writer` to define test scenarios within their own scope to support high-level bin-observable scenarios. Specialized subagents must author or update tests upfront asserting this behavior before modifying production code, confirming that tests fail before implementing logic.

---

## 2. Workspace Layout & Delegation Decision Matrix

```text
project-metaling/
├── AGENTS.md                 # [Main Agent] Master architecture & orchestration rules
├── Cargo.toml                # [Main Agent] Workspace manifest
├── .agents/
│   └── agents/               # [Main Agent] Custom subagent definitions (.md)
├── reference/                # [reference-explorer (R/O) & reference-curator (Catalog R/W)]
│   ├── AGENTS.md             # [Main Agent] Reference & knowledge catalog governance
│   ├── sync-references.sh    # [reference-curator] Clones/updates gitignored reference repos
│   └── catalog/              # [reference-curator] Version-controlled knowledge catalog (INDEX.md + domains)
├── crates/
│   ├── ro-contracts/         # [Main Agent] Shared traits, IDs, ClientIntent, OutboundPacket, DTOs
│   ├── ro-packets/           # [ro-packets Subagent] Binary wire protocol, framing, PACKETVER structs, bit-packing
│   ├── ro-engine/            # [ro-engine Subagent] ECS World, 8x8 SpatialGrid, Map tick loop, AoI, movement, entities
│   ├── ro-db/                # [ro-db Subagent] Static YAML database loaders (item_db, mob_db, skill_db) & .gat map cache
│   ├── ro-mechanics/         # [ro-mechanics Subagent] Pure stateless math functions (stats, damage Context, status rules)
│   ├── ro-persistence/       # [ro-persistence Subagent] PostgreSQL (sqlx) & stub repositories for accounts/characters
│   └── ro-scripting/         # [ro-scripting Subagent] Adapter around rathena-script-lang-interpreter for NPC/item scripts
├── bins/                     # [ro-bins Subagent] Server binary entrypoints & inter-server coordination
│   ├── login-server/         # TCP :6900 & HTTP :6901 binary entrypoint
│   ├── char-server/          # TCP :6121 & HTTP :6122 binary entrypoint
│   └── map-server/           # TCP :5121 & HTTP :5122 binary entrypoint (hosts `crates/ro-engine`)
└── tools/                    # [ro-tools Subagent] Auxiliary development & debugging tools
    ├── docker/               # Docker Compose stack for PostgreSQL and server containers
    └── proxy/                # RO packet logging & forwarding proxy (:6900, :6121, :5121)
```

### 2.1 Behavior Delegation Matrix
When implementing a feature or fixing a bug, the Main Agent uses this matrix to determine where each behavior belongs and how crates interact through `crates/ro-contracts`:

| Behavior / Responsibility | Owner Scope | Contract Interface (`crates/ro-contracts`) | Upstream Caller / Consumer |
| :--- | :--- | :--- | :--- |
| **High-level bin-observable test scenarios, domain test scenario coordination, acceptance criteria** | `spec-writer` | Test scenario specifications, observable bin assertions, domain test definitions | Main Agent, all domain crates, `bins/*`, and `tools/*` |
| **Cross-crate IDs, DTOs, Intents, Events, & Traits** | `crates/ro-contracts` (Main Agent) | `ids.rs`, `packet.rs`, `map.rs`, `http.rs`, repository & engine traits | All domain crates, `bins/*`, and `tools/*` |
| **TCP framing, opcode length table, byte parsing, event encoding, `WBUFPOS`/`WBUFPOS2` bit-packing** | `crates/ro-packets` (`ro-packets`) | `PacketLengthTable`, `PacketIntentParser`, `EventEncoder`, `RawPacket` | `bins/{login,char,map}-server/src/session.rs`, `tools/proxy` |
| **`.gat`/`.rsw` walkability grids, `item_db`/`mob_db`/`skill_db` YAML loading, spawn/warp file parsing** | `crates/ro-db` (`ro-db`) | `MapWalkability`, static blueprint DTOs, and database provider traits | Initialized by `bins/map-server`, queried by `crates/ro-engine` |
| **Derived stats (`ATK`, `DEF`, `ASPD`, `HIT`, `FLEE`), elemental/size/race modifiers, damage formulas** | `crates/ro-mechanics` (`ro-mechanics`) | Pure stateless calculator traits and `DamageContext` / stat DTOs | Invoked by `crates/ro-engine` during combat and status ticks |
| **Account authentication, character slot CRUD, PostgreSQL `sqlx` repository (`PostgresRepo`)** | `crates/ro-persistence` (`ro-persistence`) | `AccountRepository`, `CharacterRepository`, `CharacterSummary` | Injected into `LoginService`, `CharService`, and `MapService` |
| **NPC conversation execution (`mes`, `next`, `select`, `warp`) & item script evaluation** | `crates/ro-scripting` (`ro-scripting`) | Script VM runner traits and script suspension/resumption events | Invoked by `crates/ro-engine` during entity interaction phases |
| **ECS `World`, `8x8` `SpatialGrid`, A* pathfinding over `MapWalkability`, AoI calculation, entity lifecycle, deterministic tick loop** | `crates/ro-engine` (`ro-engine`) | Consumes `ClientIntent` (with session/character context), emits `ScopedServerEvent` | Hosted and ticked by `bins/map-server` (`MapService`) |
| **TCP listener loops, char-server raw 4-byte `AccountId` echo, `session.rs` channel bridge, Axum HTTP routers/clients, service dependency injection** | `bins/*` (`ro-bins`) | Consumes `crates/ro-contracts` traits and connects concrete domain types | Executed directly as server binaries |
| **RO packet logging & TCP forwarding proxy (`:6900`, `:6121`, `:5121`) and Docker Compose infrastructure (`tools/docker`)** | `tools/*` (`ro-tools`) | Consumes `PacketLengthTable` and packet framing traits; mounts SQL init files from `crates/ro-persistence` | Executed as standalone diagnostic and container tooling |

### 2.2 Multi-Subagent Delegation Sequencing
When introducing or changing behavior across crates and binaries, the Main Agent executes work in seven stages:
* **Stage 1 (Domain Consultation & Contracts First):** Consult `reference-explorer` if reference specs are needed. Consult domain subagents (`ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`, `ro-bins`, `ro-tools`) for requirements and feasibility. Update `crates/ro-contracts` with required traits, DTOs, and enum variants, and run `cargo check -p ro-contracts` to verify compilation.
* **Stage 2 (High-Level Scenario Specification via `spec-writer`):** Before authoring implementation plans, the Main Agent spawns `spec-writer` (`pro` tier) via `invoke_subagent` to draft high-level test scenarios observable from the server binaries (`bins`).
* **Stage 3 (Scoped Domain Scenario Collaboration):** Each specialized subagent collaborates with `spec-writer` to define scoped unit and integration test scenarios within its own domain to support the high-level bin scenarios.
* **Stage 4 (Architectural Scenario Review):** The Main Agent invokes `architectural-code-reviewer` (`pro` tier) via `invoke_subagent` to audit all test scenarios for overall completeness and adherence to `DESIGN.md` and `AGENTS.md`.
* **Stage 5 (Upfront Test Authoring; Red Phase):** Specialized subagents write or update unit and integration tests across bins and crates to assert the new behavior upfront before making production changes. Tests must run and fail against current code.
* **Stage 6 (Implementation & Wiring; Green Phase):** Invoke leaf crate subagents (`ro-packets`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`) and `ro-engine`, then invoke `ro-bins` (and `ro-tools` when applicable) to connect concrete providers and implement logic until all tests pass.
* **Stage 7 (Holistic Architectural Review):** Subagents invoke `code-reviewer` for multi-file crate changes. Once all tests pass and components are integrated, the Main Agent invokes `architectural-code-reviewer` (`pro` tier) to audit the end-to-end vertical slice across crates, binaries, and tools before completing the milestone.

---

## 3. Reference Repositories Routing Table (`./reference/` via `reference-explorer` & `reference-curator`)
Only the `reference-explorer` subagent may inspect the upstream repositories in `./reference/**`, and only `reference-curator` may write to `./reference/catalog/**` and `./reference/sync-references.sh`. The Main Agent and all other subagents consult `reference-explorer` to retrieve protocol specs, YAML schemas, and Aegis mechanics. `reference-explorer` queries `./reference/catalog/INDEX.md` first before scanning raw sources and invokes `reference-curator` to record new findings. **Never copy C/C++ pointer control flow or `rust-ro`'s non-ECS service layer.**

| Target Domain | Primary Reference Paths Inspected by `reference-explorer` | What `reference-explorer` Extracts |
| :--- | :--- | :--- |
| **`crates/ro-packets`** | • `reference/rathena/src/config/packets.hpp`<br>• `reference/rathena/src/map/packets_struct.hpp`<br>• `reference/rathena/src/map/packets.hpp`<br>• `reference/rathena/src/map/clif_packetdb.hpp`<br>• `reference/rathena/src/map/clif_shuffle.hpp`<br>• `reference/rathena/src/login/loginclif.cpp`<br>• `reference/rathena/src/char/char_clif.cpp`<br>• `reference/rust-ro/lib/packets/` | `PACKETVER = 20260219` struct layouts from `rathena`. Binary reader/writer patterns and 3-byte (`WBUFPOS`) / 6-byte (`WBUFPOS2`) coordinate bit-packing from `rust-ro`. Shuffled incoming map opcodes in `clif_shuffle.hpp` overriding `clif_packetdb.hpp`. |
| **`crates/ro-engine`** | • `reference/Hercules/src/map/*.h`<br>• `reference/rathena/src/map/map.cpp`<br>• `reference/rathena/src/map/pc.cpp`<br>• `reference/rathena/src/map/unit.cpp` | High-level state machine lifecycles (login spawn handshake, walking ticks, sitting, portal warp triggers, AoI broadcast ranges) to re-implement inside ECS. |
| **`crates/ro-db`** | • `reference/rathena/db/re/` (and `db/import/`)<br>• `reference/rathena/npc/re/warps/` & `mobs/`<br>• `reference/rust-ro/tools/map-cache/` | `serde_yaml` schemas for items, mobs, skills, and job tables; `.gat`/`.rsw` map cell walkability parsing; monster and warp spawn definitions. |
| **`crates/ro-mechanics`** | • `reference/rathena/src/map/battle.cpp`<br>• `reference/rathena/src/map/status.cpp`<br>• `reference/rathena/src/map/skills/<job>/*.cpp`<br>• `reference/rust-ro/tools/stat-calc/` | Pure mathematical formulas, ASPD/stat derivations, elemental/size/race tables, and `SkillImpl` ratios. |
| **`crates/ro-persistence`** | • `reference/rathena/sql-files/main.sql`<br>• `reference/rust-ro/server/src/repository/` | Account, character, and inventory relational table designs adapted for modern PostgreSQL (`sqlx`). |
| **`crates/ro-scripting`** | • `reference/rathena-script-lang-interpreter/src/` | VM hooks and native function bindings to bridge NPC scripts with ECS events. |

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
* **Extraction Tooling (No Standalone C++ Builds):** `reference-explorer` must never compile standalone C++ binaries against rAthena headers because the headers transitively require MySQL client libraries and internal subsystem headers. `reference-explorer` uses the C preprocessor directly (`clang -E -DPACKETVER=20260219 ...`) or ripgrep pattern searches over macro definitions.
* **32-bit Item IDs (`u32`):** Modern `PACKETVER` defines `MAX_ITEMID > UINT16_MAX`. All item ID fields in packets and contracts must use `u32`.
* **Graceful Unknown Packet Logging:** If the server receives an opcode not yet required for the current phase, log `tracing::debug!("Unhandled packet opcode={:#06x} len={}", opcode, bytes.len())`, consume the frame bytes using the packet length table, and continue the session loop.
* **Client-Initiated TCP Socket Lifecycle & Handoff Policy:**
  1. Connection establishment is client-initiated. All three server processes listen passively on TCP sockets.
  2. During normal state handoffs (`CA_LOGIN -> AC_ACCEPT_LOGIN`, `CH_SELECT_CHAR -> HC_NOTIFY_ZONESVR`), servers must keep the TCP socket open. The connection must remain open until the client initiates socket closure (`read_buf` returns `Ok(0)` EOF). Proactive server closure causes the official client to flag a network disconnect and abort the handoff.
  3. Server-initiated socket teardown is reserved exclusively for framing violations, unrecoverable parse errors, administrative kicks or bans, server process shutdown, and session idle timeouts.

---

## 5. Incremental Implementation Roadmap (Phases 0 to 4)

### Phase 0: Three-Server Network Handshake & Map Entry (Stub State)
* **Goal:** Connect an official client matching `PACKETVER`, log in with a hardcoded stub account/character, select the Character Server, select the character, and load onto a starter map (`prontera`).
* **Deliverables:**
  1. `login-server` (`:6900`): Handle `CA_LOGIN`, issue session token `(AccountId, LoginId1, LoginId2)`, send `AC_ACCEPT_LOGIN` with `char-server` IP/port, and keep the TCP stream open for client-initiated closure.
  2. `char-server` (`:6121`): Handle `CH_ENTER`, echo raw 4-byte `AccountId`, send `HC_ACCEPT_ENTER` (and character page packets for modern `PACKETVER`) with 1 hardcoded stub character, handle `CH_SELECT_CHAR`, reply with `HC_NOTIFY_ZONESVR` (`prontera.gat`, `map-server` IP/port), and keep the TCP stream open for client-initiated closure.
  3. `map-server` (`:5121`): Handle `CZ_ENTER`, respond with `ZC_AID` (`0x0283`) + `ZC_INVENTORY_EXPANSION_INFO` (`0x0b18`) + `ZC_ACCEPT_ENTER` (`0x02eb`, spawn coords + server tick), handle `CZ_NOTIFY_ACTORINIT` (`0x007D` "Map Loaded"), respond with initial actor state (`ZC_NOTIFY_NEWENTRY` / `ZC_PAR_CHANGE`), and reply to `CZ_REQUEST_TIME` (`0x0360`) with `ZC_NOTIFY_TIME` so the client does not time out.
  4. All other incoming packets log their opcode + payload length and are safely ignored.

### Phase 1: Map Movement & Sitting
* **Goal:** Walk around `prontera` with smooth pathing and toggle sitting/standing.
* **Deliverables:**
  1. `crates/ro-db`: Load `.gat` walkability grids for maps.
  2. `crates/ro-engine`: Implement A* pathfinding over `.gat` cells, `WalkingPath` sparse component, and `MapPosition` updates in the 8x8 `SpatialGrid`.
  3. Handle `CZ_REQUEST_MOVE` -> emit `ZC_NOTIFY_PLAYERMOVE` (6-byte bit-packed coordinates `WBUFPOS2`).
  4. Handle `CZ_CHANGE_DIRECTION` (head/body direction) and `CZ_REQUEST_ACT` (`action = 2` sit, `action = 3` stand) -> broadcast `ZC_NOTIFY_ACT` to AoI.

### Phase 2: Static Entities (Monsters & Map Portals)
* **Goal:** See monsters (stationary) and warp portal visual effects spawned on the map.
* **Deliverables:**
  1. `crates/ro-db`: Parse `mob_db.yml` and map spawn/warp definitions.
  2. `crates/ro-engine`: Spawn Monster ECS entities and Portal (`GroundSkillUnit` / warp NPC) entities into the `SpatialGrid`.
  3. Broadcast `ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY` when a player enters AoI or finishes `CZ_NOTIFY_ACTORINIT`, and handle `CZ_REQNAME` (`0x0368`) -> `ZC_ACK_REQNAME` so hovering over a monster displays its name.

### Phase 3: Map Switching & Portal Collision
* **Goal:** Walk into a warp portal cell and seamlessly transition to the destination map.
* **Deliverables:**
  1. `crates/ro-engine`: Detect when a player's `MapPosition` steps within the trigger radius of a Warp Portal entity.
  2. Despawn player from the current Map `World`, transfer entity state to the destination Map `World`, and emit `ZC_NPCACK_MAPMOVE` (`0x0091`) with the new `.gat` name and `(X, Y)` coordinates.
  3. Re-run the `CZ_NOTIFY_ACTORINIT` (`0x007D`) map-load synchronization cycle on arrival.

### Phase 4: PostgreSQL Persistence & Character Creation
* **Goal:** Replace hardcoded stubs with real PostgreSQL persistence using `sqlx`.
* **Deliverables:**
  1. `crates/ro-persistence`: Implement PostgreSQL migrations and queries for `accounts` and `characters` (persisting `account_id`, `char_id`, `name`, `slot`, `sex`, `hair_style`, `hair_color`, `last_map`, `last_x`, `last_y`).
  2. `char-server`: Implement `CH_MAKE_CHAR` (`0x0067` / `0x0A39`) and `CH_DELETE_CHAR`, persisting new characters to PostgreSQL.
  3. `map-server`: Load character spawn location/appearance from `crates/ro-persistence` on login and flush updated `(last_map, last_x, last_y)` on movement/warp/disconnect.
