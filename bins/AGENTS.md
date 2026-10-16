# `bins/` Module Rules (Server Binaries & Inter-Server Coordination)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-bins.md` (`ro-bins` custom subagent).
* **Write Scope:** `./bins/login-server/**`, `./bins/char-server/**`, and `./bins/map-server/**` exclusively.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Peer Subagent Integration:** Never read or modify implementation files inside `./crates/ro-packets/**`, `./crates/ro-engine/**`, `./crates/ro-db/**`, `./crates/ro-mechanics/**`, `./crates/ro-persistence/**`, `./crates/ro-scripting/**`, or `./tools/**`. Invoke or message the corresponding crate subagents via `invoke_subagent` or `send_message` whenever you need public constructor signatures, concrete provider types, or integration details.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever reference handshake or session lifecycle behavior from `./reference/**` is needed. Never read `./reference/**` directly.
* **Multi-File Review Gate:** Any change spanning more than one file in `./bins/**` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit actor concurrency, handshake sequencing, and HTTP edge cases before completion.

---

## 2. Binary Restraint & Domain Delegation Rules
1. **Zero Inline Domain State in `bins/*/src/service.rs`:**
   * `LoginService` (`bins/login-server`) and `CharService` (`bins/char-server`) must delegate account lookups, credential verification, and character CRUD operations to `crates/ro-persistence` via `AccountRepository` and `CharacterRepository`.
   * `MapService` (`bins/map-server`) must delegate map state, spawn coordinates, entity tracking, server tick generation, movement, and AoI event emission to `crates/ro-engine`. `MapService` runs the tick timer, feeds session-authenticated intents into `crates/ro-engine`, coordinates map transfers, and routes emitted `ScopedServerEvent` payloads to client sessions.
2. **Protocol Framing vs. Session Context:**
   * `crates/ro-packets` owns wire-level framing and opcode-to-struct decoding and encoding.
   * `bins/*/src/session.rs` owns the live `TcpStream`, the raw 4-byte `AccountId` handshake echo required by `CH_ENTER` and `CZ_ENTER`, and per-connection session state (`SessionId`, authenticated `AccountId`, and `CharId`).

---

## 3. Network Topology & Protocol-to-Logic Channel Bridge

### 3.1 Client TCP Boundaries
* **`login-server` (TCP `:6900`):** Accepts transient connections, decodes `CA_LOGIN`, issues a `SessionTicket`, returns `AC_ACCEPT_LOGIN` containing available character servers, and closes the connection.
* **`char-server` (TCP `:6121`):** Accepts transient connections, decodes `CH_ENTER`, verifies the `SessionTicket`, echoes the 4-byte `AccountId`, returns `HC_ACCEPT_ENTER` and character list packets, handles `CH_SELECT_CHAR` / `CH_MAKE_CHAR` / `CH_DELETE_CHAR`, returns `HC_NOTIFY_ZONESVR` pointing to `map-server`, and closes the connection.
* **`map-server` (TCP `:5121`):** Accepts persistent gameplay connections, decodes `CZ_ENTER`, echoes the 4-byte `AccountId`, returns `ZC_ACCEPT_ENTER`, handles `CZ_NOTIFY_ACTORINIT` and `CZ_REQUEST_TIME`, and broadcasts AoI events.

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
   * Queues the raw 4-byte `AccountId` payload to the outbound writer immediately upon receiving `ClientIntent::EnterCharServer` or `ClientIntent::EnterMap`.
2. **Non-Blocking Outbound Dispatch & Actor Lifetimes:**
   * Logic actors must use non-blocking `try_send` to push outbound bytes or events to a session's outbound writer queue, disconnecting overflowed sessions so a slow client never stalls shared loops.
   * `login-server` spawns an ephemeral `LoginSessionActor` per connection.
   * `char-server` spawns an ephemeral `CharSessionActor` per connection.
   * `map-server` runs a long-lived `MapWorldActor` per map instance owning a `ro-engine` `World` and deterministic tick timer.
