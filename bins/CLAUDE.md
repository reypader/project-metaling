# Server binaries

Keep binaries thin: config, wiring, TCP session supervision, internal HTTP (DTOs in `ro-contracts/src/http.rs`), and actor channels. Game logic belongs in `crates/`.

## Socket lifecycle invariant (the official client depends on this)

- Clients open every connection. Servers only listen.
- After a successful handoff (`AC_ACCEPT_LOGIN` 0x0ac4 from login, `HC_NOTIFY_ZONESVR` 0x0ac5 from char), the server **must keep the socket open** and keep its reader loop running until the client sends EOF (`read_buf` → `Ok(0)`) or an I/O error happens. If the server closes first (`shutdown()`, dropping the session task), the client reports a disconnect and aborts the handoff.
- The server may close a connection only for: framing corruption, unrecoverable parse errors, admin kick/ban, process shutdown, or idle timeout.

## Raw AccountId echo

Right after `CH_ENTER` (char) and `CZ_ENTER` (map), the server sends the 4-byte `AccountId` with **no opcode**. This byte stream is not framed. The proxy and any stream parser must skip it before normal opcode framing resumes.

## Session identity

Session identity (`SessionId`, `AccountId`, `CharId`) is set on the session in `session.rs` after the handshake. Only handshake packets carry IDs on the wire, so attach these IDs to each intent there.
