# Login Server Packets (`CA_*` / `AC_*`)

Target client version: `PACKETVER = 20260219`. Packet entries are sorted by ascending hexadecimal opcode.

## TCP Socket Lifecycle and Handoff Policy

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 4) & `rathena` (`src/login/loginclif.cpp`, `PACKETVER = 20260219`).
* **Connection Establishment:** All connections are initiated by the client. The login server listens passively on its configured TCP port (`:6900`).
* **Handoff Retention:** After receiving `CA_LOGIN` and transmitting `AC_ACCEPT_LOGIN` (`0x0ac4`), the login server keeps the TCP stream open. The server waits for the client to close the socket (`read_buf` returning `Ok(0)` EOF) before cleaning up the session. Closing the socket proactively from the server side causes the official client to report a network disconnection and abort the transition to the character server.
* **Permitted Server Teardown:** The server initiates socket closure only upon framing violations, unrecoverable parse errors, administrative kicks or bans, server process shutdown, or session idle timeouts.

## Packet Specifications

### `0x0ac4` `AC_ACCEPT_LOGIN`

* **Opcode:** `0x0ac4`
* **Canonical Name:** `AC_ACCEPT_LOGIN` (`PACKET_AC_ACCEPT_LOGIN3`)
* **Direction:** Server to Client (Login Server)
* **Frame Length:** Dynamic (`64 + 160 * N` bytes, where `N` is the number of character server entries)
* **Provenance:** `rathena` (`src/common/packets.hpp`, `src/login/loginclif.cpp`), `PACKETVER = 20260219` (`PACKETVER >= 20170315`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0ac4` (little-endian) |
| `2` | `2` | `u16` | `packet_len` | Total byte length (`64 + 160 * char_servers.len()`) |
| `4` | `4` | `u32` | `login_id1` | Session authentication token 1 |
| `8` | `4` | `u32` | `account_id` | Assigned Account ID (`AID`) |
| `12` | `4` | `u32` | `login_id2` | Session authentication token 2 |
| `16` | `4` | `u32` | `last_ip` | Last login IPv4 address (`0` if unused) |
| `20` | `26` | `[u8; 26]` | `last_login` | Null-terminated last login timestamp string |
| `46` | `1` | `u8` | `sex` | Account gender (`0` = Female, `1` = Male, `2` = Server) |
| `47` | `17` | `[u8; 17]` | `web_auth_token` | Web authentication token (zero-padded for stub/offline mode) |
| `64` | `160 * N` | `[ServerEntry; N]` | `char_servers` | Repeating 160-byte character server descriptors (IP, port, name, user count, state, property, unknown padding) |
