# AGENTS.md: Project Metaling (Root Orchestrator & Architecture Spec)

## 1. Project Vision & Task-Oriented Operating Model
**Project Metaling** is a clean-room *Ragnarok Online* (RO) server workspace written in **Rust** using a custom **Entity-Component-System (ECS)** architecture.

The workspace organizes AI agent labor around **tasks and concrete deliverables** rather than isolated domain silos. Subagents read files directly from disk across the repository as necessary for their work. Direct disk access eliminates the context duplication, token bloat, and session delays caused by inter-agent messaging.

### Role 1: The Main Agent (Root Orchestrator & Contract Arbiter)
Operating at the workspace root, the **Main Agent** manages architecture, task coordination, and delivery:
* **Exclusive Management Scope:**
  * `./Cargo.toml` (Workspace manifest and workspace-wide dependencies).
  * `./AGENTS.md`, `./crates/*/AGENTS.md`, `./bins/AGENTS.md`, `./tools/AGENTS.md`, `./reference/AGENTS.md`, `./specs/AGENTS.md`, and `./.agents/**` (Governance, module rules, and task-oriented subagent definitions).
  * `./specs/**` (Read access to `spec.md`; exclusive authoring of `./specs/<feature-or-phase>/plan.md` and `./specs/AGENTS.md`).
  * `./crates/ro-contracts/**` (Cross-crate traits, DTOs, Event enums, error types, and ID newtypes).
* **Core Responsibilities:**
  1. Consult `reference-explorer` or `./reference/catalog/` whenever upstream specifications are required to design contracts or features.
  2. Maintain interface purity in `crates/ro-contracts`, adding or updating traits and DTOs and verifying with `cargo check -p ro-contracts`.
  3. Coordinate the test-driven development workflow:
     * Spawn `spec-writer` via `invoke_subagent` to author `./specs/<feature-or-phase>/spec.md` with observable integration scenarios and component test criteria.
     * Invoke `architectural-code-reviewer` to audit `./specs/<feature-or-phase>/spec.md` for design completeness, contract alignment, and recommended milestone decompositions.
     * Author `./specs/<feature-or-phase>/plan.md` mapping implementation steps to scenario IDs and line ranges in `spec.md`, structuring work into the recommended milestones.
     * Direct `implementer` to author failing tests upfront and implement production code until all tests pass.
     * Direct `architectural-code-reviewer` to audit the completed end-to-end vertical slice.
  4. Delegate all scratch scripts, data parsing, format conversions, and bulk edits to `utility-scripter`.
  5. Delegate external web searches and documentation retrieval to `web-researcher`.

#### Mandatory Operating Rules for the Main Agent:
1. **Interface Purity in `crates/ro-contracts`:**
   `crates/ro-contracts` contains zero business logic, zero concrete codecs, zero wire framing loops, and zero heavy transport dependencies. Only abstract data structures, enums, newtypes, and traits belong in this crate.
2. **Subagent Delegation:**
   The Main Agent delegates code implementation, spec authoring, scratch scripting, and reviews to specialized task-oriented subagents via `invoke_subagent`.
3. **File-Based Red-Green Testing Protocol:**
   Whenever introducing or modifying behavior, acceptance criteria and tests must be established in `spec.md` and written upfront before modifying production code.
4. **Mandatory Scripting Delegation (`utility-scripter`):**
   The Main Agent must never author, generate, or execute temporary scratch scripts, one-off file parsers, data extraction scripts, or bulk format converters directly. Delegate all scripting tasks to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.
5. **Mandatory Web Research Delegation (`web-researcher`):**
   The Main Agent must never perform web searches or link crawling directly. Delegate external web research to `web-researcher`.

### Role 2: Task-Oriented Subagents (`./.agents/agents/*.md`)
The workspace configures seven task-oriented subagents defined by expected outcomes and deliverables:

| Subagent Name | Config File | Task & Primary Deliverable | Operating Scope |
| :--- | :--- | :--- | :--- |
| **`spec-writer`** | `.agents/agents/spec-writer.md` | **Task:** Authors test scenario specifications.<br>**Deliverable:** Approved `./specs/<feature-or-phase>/spec.md` covering observable scenarios and component test criteria. | Reads workspace-wide across all crates, bins, tools, reference catalogs, and specs. Writes `./specs/<feature-or-phase>/spec.md` only. |
| **`implementer`** | `.agents/agents/implementer.md` | **Task:** Clean-room Rust implementation and testing.<br>**Deliverable:** Passing unit/integration tests and production code across crates, bins, and tools meeting `spec.md` and `plan.md`. | Reads and writes across `./crates/**`, `./bins/**`, `./tools/**`, and `./Cargo.toml` as needed for the implementation task. |
| **`code-reviewer`** | `.agents/agents/code-reviewer.md` | **Task:** Audits multi-file code changes within packages.<br>**Deliverable:** Structured review report with line-specific findings and verdict (`APPROVED` or `CHANGES_REQUIRED`). | Strictly read-only across the workspace. Outputs review outcomes only. Exempts `utility-scripter` scripts. |
| **`architectural-code-reviewer`** | `.agents/agents/architectural-code-reviewer.md` | **Task:** Audits specifications, vertical slices, and recommends milestone decompositions.<br>**Deliverable:** Structured architectural review report with verdict (`APPROVED` or `CHANGES_REQUIRED`) and milestone recommendations. | Strictly read-only across the workspace. Outputs review outcomes only. Exempts `utility-scripter` scripts. |
| **`reference-explorer`** | `.agents/agents/reference-explorer.md` | **Task:** Reference research and knowledge base curation.<br>**Deliverable:** Curated specifications in `./reference/catalog/`, updated `INDEX.md`, and structured extraction summaries. | Reads `./reference/**`, catalog, and project files. Writes `./reference/catalog/**` and `./reference/sync-references.sh`. |
| **`utility-scripter`** | `.agents/agents/utility-scripter.md` | **Task:** Scratch scripting, parsing, and format conversions.<br>**Deliverable:** Tested script output, processed datasets, and converted files returned to requester. | Operates in scratch directories and delegated data files. Exempt from code and architectural reviews. |
| **`web-researcher`** | `.agents/agents/web-researcher.md` | **Task:** External web and documentation research.<br>**Deliverable:** Structured summaries with citations, API signatures, and relevant code excerpts. | Web search and URL fetching. Reads project contracts for alignment. |

#### Mandatory Operating Rules for All Subagents:
1. **Direct Disk Inspection:**
   All subagents read project files directly from disk across all directories. Never use inter-agent messaging to discover public signatures, constructors, or type definitions.
2. **Reviewer & Spec Writer Restraint:**
   Reviewers (`code-reviewer`, `architectural-code-reviewer`) are strictly read-only and output review outcomes only. `spec-writer` writes only `./specs/<feature-or-phase>/spec.md`. Neither reviewers nor `spec-writer` write production or test code.
3. **Mandatory Scripting Delegation (`utility-scripter`):**
   All subagents that require one-off scripts, parsers, macro extractions, or bulk format conversions must delegate script creation to `utility-scripter`. The requesting subagent describes the requirement, available input, and necessary output. `utility-scripter` builds and tests the script before returning output. Utility scripting is exempt from any form of code or architectural review.
4. **Mandatory Multi-File Code Review Gate:**
   Whenever `implementer` modifies more than one file in a crate, binary, or tool package, it must invoke `code-reviewer` and resolve all findings before completing the task.
5. **Local Module Rules (`AGENTS.md`):**
   When touching code inside a module (`crates/ro-engine`, `crates/ro-packets`, `bins/login-server`, etc.), `implementer` and reviewers must consult that module's local `AGENTS.md` and follow its architectural constraints and verification commands.

---

## 2. Workspace Layout & Task-Based Delegation Matrix

```text
project-metaling/
├── AGENTS.md                 # Master architecture & task-oriented orchestration rules
├── Cargo.toml                # Workspace manifest
├── .agents/
│   └── agents/               # Custom subagent definitions (.md)
├── specs/                    # [spec-writer (spec.md R/W), Main Agent (plan.md R/W)]
│   ├── AGENTS.md             # Specification governance and templates
│   └── <feature-or-phase>/   # Per-feature specification and implementation plan
├── reference/                # [reference-explorer (R/W for catalog, R/O for upstream)]
│   ├── AGENTS.md             # Reference governance & catalog rules
│   ├── sync-references.sh    # Upstream repository clone/update script
│   └── catalog/              # Version-controlled knowledge catalog (INDEX.md + domains)
├── crates/
│   ├── ro-contracts/         # Pure cross-crate traits, IDs, intents, events, DTOs
│   ├── ro-packets/           # Binary wire protocol, framing, PACKETVER codecs, bit-packing
│   ├── ro-engine/            # Deterministic ECS World, SpatialGrid, movement, tick loop
│   ├── ro-db/                # Static YAML database loaders & .gat walkability grids
│   ├── ro-mechanics/         # Pure stateless Aegis and Renewal formulas & modifier tables
│   ├── ro-persistence/       # PostgreSQL (sqlx) & stub repositories for accounts/characters
│   └── ro-scripting/         # Non-blocking rAthena script VM adapter
├── bins/                     # Server binary entrypoints & inter-server coordination
│   ├── login-server/         # TCP :6900 & HTTP :6901 binary entrypoint
│   ├── char-server/          # TCP :6121 & HTTP :6122 binary entrypoint
│   └── map-server/           # TCP :5121 & HTTP :5122 binary entrypoint
└── tools/                    # Standalone development utilities & infrastructure
    ├── docker/               # PostgreSQL container configuration
    └── proxy/                # Packet logging & forwarding proxy (:6900, :6121, :5121)
```

### 2.1 Task Delegation Matrix

| Task / Responsibility | Owner Agent | Deliverable | Primary Inputs |
| :--- | :--- | :--- | :--- |
| **Scenario Specification Authoring** | `spec-writer` | `./specs/<feature-or-phase>/spec.md` | Feature request, `crates/ro-contracts`, codebase state, `./reference/catalog/` |
| **Specification Audit & Milestone Breakdown** | `architectural-code-reviewer` | Review verdict (`APPROVED` / `CHANGES_REQUIRED`) and milestone recommendations | `spec.md`, `DESIGN.md`, `AGENTS.md`, `crates/ro-contracts` |
| **Implementation Plan Authoring** | Main Agent | `./specs/<feature-or-phase>/plan.md` | Approved `spec.md`, task priorities |
| **Upfront Test & Code Implementation** | `implementer` | Passing test suites & production code | `spec.md`, `plan.md`, module `AGENTS.md` files |
| **Multi-File Code Review** | `code-reviewer` | Review report (`APPROVED` / `CHANGES_REQUIRED`) | Modified files, `spec.md`, `plan.md`, module `AGENTS.md` |
| **End-to-End Vertical Slice Audit** | `architectural-code-reviewer` | Architectural report (`APPROVED` / `CHANGES_REQUIRED`) | Integrated codebase, passing test runs, `spec.md`, `plan.md` |
| **Reference Research & Curation** | `reference-explorer` | Catalog entries in `reference/catalog/` & summaries | Upstream repositories (`rathena`, `Hercules`, `rust-ro`, `interpreter`) |
| **One-Off Scripting & Parsing** | `utility-scripter` | Verified script output & processed data | Calling agent description, input datasets (exempt from review) |
| **External Documentation Research** | `web-researcher` | Structured summaries with URLs & code samples | Official web documentation, RFCs, remote repositories |

### 2.2 Task-Oriented Execution Workflow
When introducing or modifying behavior across crates and binaries, work proceeds through six stages:
* **Stage 1 (Contract Touchpoints & Reference Lookup):** If reference specifications are needed, consult `reference-explorer` or `./reference/catalog/`. Update `crates/ro-contracts` with required traits, DTOs, and enum variants, verifying compilation via `cargo check -p ro-contracts`.
* **Stage 2 (Scenario Specification via `spec-writer`):** The Main Agent invokes `spec-writer` via `invoke_subagent`. `spec-writer` reads contracts, existing tests, and module code directly from disk to author `./specs/<feature-or-phase>/spec.md` with high-level observable scenarios and component test criteria.
* **Stage 3 (Architectural Scenario Audit & Milestone Breakdown):** `spec-writer` or the Main Agent invokes `architectural-code-reviewer` to audit `./specs/<feature-or-phase>/spec.md`. The reviewer audits design completeness and recommends incremental task or milestone breakdowns so implementers avoid overly large deliverables. Recommended breakdowns can be motivated by architectural refactoring for emergent patterns, abstraction for extendability, or independent behaviors. Milestone division leverages stubs in code to maintain compilation while delineating milestone boundaries. Once approved, the Main Agent writes the execution steps in `./specs/<feature-or-phase>/plan.md`.
* **Stage 4 (Upfront Test Authoring; Red Phase):** The Main Agent directs `implementer` to author or update unit, integration, and wire tests upfront according to `spec.md` and `plan.md`. Tests must run and fail against current code.
* **Stage 5 (Implementation & Verification; Green Phase):** `implementer` implements production code across the target crates, binaries, or tools until all tests pass cleanly. `implementer` invokes `code-reviewer` on multi-file changes to resolve any defects.
* **Stage 6 (Vertical Slice Architectural Audit):** Once all tests pass and components integrate, the Main Agent invokes `architectural-code-reviewer` to audit the complete vertical slice against `spec.md` and `plan.md`.

---

## 3. Reference Repositories Routing Table (`./reference/` via `reference-explorer`)
The `reference-explorer` subagent investigates upstream reference repositories and curates `./reference/catalog/**` and `./reference/sync-references.sh`. Other agents read `./reference/catalog/` directly or consult `reference-explorer`. Never copy C/C++ pointer control flow or `rust-ro` non-ECS service architecture.

| Target Module | Primary Reference Paths | What `reference-explorer` Extracts |
| :--- | :--- | :--- |
| **`crates/ro-packets`** | • `reference/rathena/src/config/packets.hpp`<br>• `reference/rathena/src/map/packets_struct.hpp`<br>• `reference/rathena/src/map/packets.hpp`<br>• `reference/rathena/src/map/clif_packetdb.hpp`<br>• `reference/rathena/src/map/clif_shuffle.hpp`<br>• `reference/rathena/src/login/loginclif.cpp`<br>• `reference/rathena/src/char/char_clif.cpp`<br>• `reference/rust-ro/lib/packets/` | `PACKETVER = 20260219` struct layouts from `rathena`. Binary reader/writer patterns and 3-byte (`WBUFPOS`) / 6-byte (`WBUFPOS2`) coordinate bit-packing from `rust-ro`. Shuffled incoming map opcodes in `clif_shuffle.hpp` overriding `clif_packetdb.hpp`. |
| **`crates/ro-engine`** | • `reference/Hercules/src/map/*.h`<br>• `reference/rathena/src/map/map.cpp`<br>• `reference/rathena/src/map/pc.cpp`<br>• `reference/rathena/src/map/unit.cpp` | State machine lifecycles (login spawn handshake, walking ticks, sitting, portal warp triggers, AoI broadcast ranges) to re-implement inside ECS. |
| **`crates/ro-db`** | • `reference/rathena/db/re/` (and `db/import/`)<br>• `reference/rathena/npc/re/warps/` & `mobs/`<br>• `reference/rust-ro/tools/map-cache/` | `serde_yaml` schemas for items, mobs, skills, and job tables; `.gat`/`.rsw` map cell walkability parsing; monster and warp spawn definitions. |
| **`crates/ro-mechanics`** | • `reference/rathena/src/map/battle.cpp`<br>• `reference/rathena/src/map/status.cpp`<br>• `reference/rathena/src/map/skills/<job>/*.cpp`<br>• `reference/rust-ro/tools/stat-calc/` | Pure mathematical formulas, ASPD/stat derivations, elemental/size/race tables, and `SkillImpl` ratios. |
| **`crates/ro-persistence`** | • `reference/rathena/sql-files/main.sql`<br>• `reference/rust-ro/server/src/repository/` | Account, character, and inventory relational table designs adapted for PostgreSQL (`sqlx`). |
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
