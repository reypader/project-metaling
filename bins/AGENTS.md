# `bins/` Module Rules (Server Binaries & Inter-Server Coordination)

## 1. Module Scope & Operational Guidelines
* **Role & Scope:** Governs the three server executables under `./bins/**` (`./bins/login-server/**`, `./bins/char-server/**`, and `./bins/map-server/**`) and inter-server communication.
* **Direct Inspection:** Agents working on `./bins/**` read all library crates (`./crates/**`), contracts, tools, specs, and reference catalogs directly from disk. Never use inter-agent messaging to discover public signatures, constructors, or type definitions.
* **Binary Restraint:** Keep binary entrypoints restricted to bootstrap configuration, dependency injection, Axum HTTP routing, TCP session supervision, raw 4-byte `AccountId` handshake echoes, and actor channel coordination (`LoginSessionActor`, `CharSessionActor`, `MapWorldActor`). Inline gameplay state machines, bespoke wire decoders, and hardcoded entity repositories inside `./bins/**` are prohibited.
* **Contract Compliance:** Inspect `./crates/ro-contracts/src/**` directly from disk. If a shared contract requires modification, update it according to task requirements and verify with `cargo check -p ro-contracts`.
* **Reference Access:** Consult `./reference/catalog/` or the `reference-explorer` subagent whenever protocol or handshake specifications are required.
* **Multi-File Review Gate:** Any change spanning more than one file in `./bins/**` requires invoking `code-reviewer` to audit actor concurrency, handshake sequencing, and HTTP edge cases before completion.
* **Verification Commands:** Run `cargo test --bins` or package-specific tests (`cargo test -p login-server`, `cargo test -p char-server`, `cargo test -p map-server`) to verify compilation and test passage.
* **Scripting Delegation:** Delegate one-off scripts, parsers, and data transformers to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.

---

## 2. Binary Restraint & Domain Delegation Rules
1. **Zero Inline Domain State in `bins/*/src/service.rs`:**
   * `LoginService` (`bins/login-server`) and `CharService` (`bins/char-server`) must delegate account lookups, credential verification, and character CRUD operations to `crates/ro-persistence` via `AccountRepository` and `CharacterRepository`.
   * `MapService` (`bins/map-server`) must delegate map state, spawn coordinates, entity tracking, server tick generation, movement, and AoI event emission to `crates/ro-engine`. `MapService` runs the tick timer, feeds session-authenticated intents into `crates/ro-engine`, coordinates map transfers, and routes emitted `ScopedServerEvent` payloads to client sessions.
2. **Protocol Framing vs. Session Context:**
   * `crates/ro-packets` owns wire-level framing and opcode-to-struct decoding and encoding.
   * `bins/*/src/session.rs` owns the live `TcpStream`, the char-server raw 4-byte `AccountId` handshake echo required by `CH_ENTER` (map-server uses framed `ZC_AID` 0x0283), and per-connection session state (`SessionId`, authenticated `AccountId`, and `CharId`).

---

## 3. Network Topology & Protocol-to-Logic Channel Bridge

### 3.1 Client TCP Boundaries
* **`login-server` (TCP `:6900`):** Accepts connections, decodes `CA_LOGIN`, issues a `SessionTicket`, returns `AC_ACCEPT_LOGIN` containing available character servers, and keeps the TCP connection open until the client initiates socket closure.
* **`char-server` (TCP `:6121`):** Accepts connections, decodes `CH_ENTER`, verifies the `SessionTicket`, echoes the 4-byte `AccountId`, returns `HC_ACCEPT_ENTER` and character list packets, handles `CH_SELECT_CHAR` / `CH_MAKE_CHAR` / `CH_DELETE_CHAR`, returns `HC_NOTIFY_ZONESVR` pointing to `map-server`, and keeps the TCP connection open until the client initiates socket closure.
* **`map-server` (TCP `:5121`):** Accepts gameplay connections, decodes `CZ_ENTER`, responds with `ZC_AID` (`0x0283`) + `ZC_INVENTORY_EXPANSION_INFO` (`0x0b18`) + `ZC_ACCEPT_ENTER` (`0x02eb`), handles `CZ_NOTIFY_ACTORINIT` and `CZ_REQUEST_TIME`, and broadcasts AoI events.

### 3.2 Inter-Server Internal Axum HTTP APIs
Server binaries coordinate over internal HTTP endpoints using JSON payloads from `crates/ro-contracts/src/http.rs`:
* **Login Server Internal API (`:6901`):**
  * `POST /api/v1/sessions/validate`: Validates an active `SessionTicket` presented to `char-server`.
  * `POST /api/v1/servers/register`: Registers a `char-server` instance and updates its capacity.
  * `POST /api/v1/servers/heartbeat`: Receives periodic health and user count telemetry.
  * `GET /api/v1/health`: Health probe endpoint.
* **Char Server Internal API (`:6122`):**
  * `POST /api/v1/maps/register`: Registers a `map-server` instance and its hosted maps.
  * `POST /api/v1/transfers/prepare`: Coordinates character transfer reservations prior to client redirection.
  * `GET /api/v1/health`: Health probe endpoint.
* **Map Server Internal API (`:5122`):**
  * `POST /api/v1/transfers/accept`: Confirms an incoming player transfer ticket.
  * `GET /api/v1/health`: Health probe reporting map instances, player count, and tick rate.

### 3.3 Unified Protocol-to-Logic Channel Bridge (`session.rs` & Actors)
1. **Protocol Worker (`session.rs`):**
   * Splits `TcpStream` into `OwnedReadHalf` and `OwnedWriteHalf` so inbound packet decoding and outbound socket writes never block each other.
   * Uses `PacketDecoder` and `PacketIntentParser` to decode incoming bytes into `ClientIntent` and wraps them in a `SessionIntent` envelope containing `(SessionId, Option<AccountId>, Option<CharId>, ClientIntent)`.
   * Rejects post-handshake intents until the session completes `EnterCharServer` or `EnterMap`.
   * Queues the raw 4-byte `AccountId` payload to the outbound writer immediately upon receiving `ClientIntent::EnterCharServer` (char-server only; map-server uses framed `ZC_AID` 0x0283 via event encoder).
2. **Non-Blocking Outbound Dispatch & Actor Lifetimes:**
   * Logic actors must use non-blocking `try_send` to push outbound bytes or events to a session's outbound writer queue, disconnecting overflowed sessions so a slow client never stalls shared loops.
   * `login-server` spawns an ephemeral `LoginSessionActor` per connection.
   * `char-server` spawns an ephemeral `CharSessionActor` per connection.
   * `map-server` runs a long-lived `MapWorldActor` per map instance owning a `ro-engine` `World` and deterministic tick timer.

### 3.4 Client-Initiated Socket Lifecycle & Handoff Invariant
1. **Client-Initiated Connection Establishment:** All server binaries bind to passive TCP listener sockets. The client initiates all outbound TCP handshakes.
2. **Client-Initiated Handoff Teardown:** During normal inter-server state transitions, the client initiates connection teardown.
   * After transmitting `AC_ACCEPT_LOGIN` (`0x0ac4`), [`login-server`](file:///Users/rmpader/code_projects/metaling/bins/login-server) must keep the TCP stream open. The client closes the socket after the user confirms a character server selection.
   * After transmitting `HC_NOTIFY_ZONESVR` (`0x0ac5`), [`char-server`](file:///Users/rmpader/code_projects/metaling/bins/char-server) must keep the TCP stream open. The client closes the socket after parsing zone credentials and transitioning to [`map-server`](file:///Users/rmpader/code_projects/metaling/bins/map-server).
   * Server binaries must never proactively close the TCP socket (`writer.shutdown()`, `set_eof`, or dropping session workers) upon completing a successful handoff. Proactive server closure causes the official client to detect a network disconnect and abort the handoff.
   * Session tasks in `bins/*/src/session.rs` must maintain the reader loop until the client transmits an EOF (`read_buf` returns `Ok(0)`) or an I/O error occurs.
3. **Permitted Server-Initiated Socket Teardown:** Servers terminate client connections exclusively on framing corruption, unrecoverable packet parse failures, administrative kicks or bans, server process shutdown, and session idle timeouts.
