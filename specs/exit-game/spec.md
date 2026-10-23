# Specification: Exit to Windows

When a player picks "Exit to Windows", the client sends `CZ_REQ_DISCONNECT` (`0x018a`) to the map-server and waits for `ZC_ACK_REQ_DISCONNECT` (`0x018b`). Before this feature the map-server ignored the request, so the client never exited. Seen in a local Metaling capture from 2026-10-23, packet #156.

Packet layouts and rAthena behaviour: [`reference/catalog/packets/map.md#0x018a-cz_req_disconnect`](../../reference/catalog/packets/map.md#0x018a-cz_req_disconnect).

## 0. What the captures settled

rAthena captures: `packet_sequence-20261023_031902.txt` L883–887, `packet_sequence-20261023_030246.txt` L894–895, and `login_sequence-rathena.txt` L215–216.

- The client sends `0x018a` with payload `00 00`, and the server replies `0x018b` with payload `00 00` about 25–70 ms later.
- About 13 ms after the ack, observers get `0x0080 ZC_NOTIFY_VANISH` with type `2` (logged out).
- Nothing else is sent. With rAthena's default config (`drop_connection_on_quit: no`), the server leaves the socket open and the client closes it.

## 1. Design

### `ro-core`
- `ClientIntent::RequestDisconnect { disconnect_type: u16 }`.
- `ServerEvent::DisconnectAck { result: u16 }`. Result 0 means the exit is allowed; 1 means it is refused ("wait 10 seconds").

### `ro-packets`
- `CzReqDisconnectPacket` decodes `0x018a`: a 4-byte frame with a `u16` type at offset 2.
- `encode_zc_ack_req_disconnect(result)` builds the 4-byte `0x018b` frame.
- `0x018b` is listed in `is_implemented_outbound_opcode`, so the proxy tags it `[IMPLEMENTED]`.

### `map-server`
- `MapWorldActor::player_intent` handles `RequestDisconnect` the same way as `Restart`:
  1. It queues `DisconnectAck { result: 0 }` for the session.
  2. It calls `leave(session, Departure::ClientCloses)`.
- `Departure::Restart` is renamed `Departure::ClientCloses`, because restart and exit now share it.
- `remove_session` already does the rest, in this order:
  - sends the ack, so the ack goes out before the observers' vanish;
  - keeps the sender in `detached`, so the server does not close first (`bins/CLAUDE.md`);
  - records the last position on the claim;
  - removes the character, which sends observers a vanish;
  - drops the claim, which queues the position write and releases the character.
- If the ack can't be queued because the outbound queue is full or closed, the sender is dropped rather than kept in `detached`. The session then closes instead of leaving the client waiting forever. Restart works the same way.
- The disconnect type is not checked, which matches rAthena.
- No change to the engine, char-server or login-server. The only online set is the map-server's claim set.

## 2. Scenarios

### PKT-QUIT-01: Decode `CZ_REQ_DISCONNECT`
`0x018a` with payload `00 00` decodes, through both `decode_frame` and the intent parser, to `RequestDisconnect { disconnect_type: 0 }`. A 1-byte payload returns `TruncatedPayload { expected: 2, actual: 1 }`.

### PKT-QUIT-02: Encode `ZC_ACK_REQ_DISCONNECT`
`DisconnectAck { result: 0 }` encodes to `8b 01 00 00`, and `is_implemented_outbound_opcode(0x018b)` is true.

### BIN-QUIT-01: Ack, then the client closes
A joined client sends `0x018a` and receives `0x018b` with payload `[0, 0]`. The server does not close the socket within 400 ms, and the character is no longer on the map.

### BIN-QUIT-02: Observer sees a logout
An observer in view receives `0x0080` for the quitting character, with vanish type `2`.

### BIN-QUIT-03: Position saved, character released
A character walks to (102, 100), exits, and closes the socket. The stored position becomes `prontera.gat` (102, 100). The same character can then enter again, and it spawns at (102, 100).

## 3. Deferred
- **Refusal (result 1).** rAthena refuses while the character is hidden or cloaked, or within `prevent_logout` (10 s by default) of attacking, casting or taking damage. On a refused exit the client stays in game. The same check also refuses `CZ_RESTART`. Add this with combat.
- **Delayed removal.** If the client closes inside the `prevent_logout` window, rAthena keeps the character in the world for 10 s. This depends on combat too.
- **Server-side close (`drop_connection_on_quit`).** rAthena's default is off, so it is not needed.
- **Quit racing a cross-map handoff.** Suppose map A hands the session off in the same moment the client sends `0x018a`. The session task can still forward the quit to map A, which no longer knows the session and drops it, so no ack is sent. `CZ_RESTART` and every other intent have the same window. For now map A logs a lost restart or exit at `warn`. The fix belongs in the handoff: map A could keep handed-off session ids for one tick and forward their intents. Its test: warp and quit in the same tick, then assert that `0x018b` arrives.
