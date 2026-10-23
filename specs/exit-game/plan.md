# Implementation Plan: Exit to Windows

Spec: `specs/exit-game/spec.md`.

## Decisions
| Topic | Decision |
| :--- | :--- |
| Ack result | Always `0`. Refusal (result `1`) waits for combat (spec §3). |
| Disconnect type | Not checked, as in rAthena. |
| Socket close | The client closes it. The server keeps the sender in `detached`, as it does for restart (`bins/CLAUDE.md`). |

## Milestones
Tests are named with spec IDs.

- [x] **M0 research:** catalog entries for `0x018a` and `0x018b` in `reference/catalog/packets/map.md`, plus their `INDEX.md` rows.
- [x] **M1 ro-core:** `ClientIntent::RequestDisconnect`, `ServerEvent::DisconnectAck`.
- [x] **M2 ro-packets:** `CzReqDisconnectPacket`, the `0x018a` frame arm, `encode_zc_ack_req_disconnect`, the encoder arm and the implemented-opcode entry. Tests: PKT-QUIT-01, PKT-QUIT-02.
- [x] **M3 map-server:** handle `RequestDisconnect` in `player_intent`, and rename `Departure::Restart` to `ClientCloses`. Tests: BIN-QUIT-01..03 in `bins/map-server/tests/map_handshake_tests.rs`.
- [x] **M4 review:** `./verify.sh --smoke` passes. The reviewer pass is done, with these changes:
  - A failed ack send now drops the sender instead of keeping it in `detached`.
  - A restart or exit from an unjoined session is logged at `warn`.
  - BIN-QUIT-03 checks the save before the socket closes.
  - BIN-QUIT-01 and BIN-QUIT-02 assert that nothing else reaches the quitter.
  - The handoff race is deferred (spec §3).
- [ ] **M5 real client:** through the proxy, "Exit to Windows" closes the client, the observer sees the vanish, and the character logs back in at the cell where it quit.
