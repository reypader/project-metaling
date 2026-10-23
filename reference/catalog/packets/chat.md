# Chat Packets (public/global chat)

Target: `PACKETVER = 20260219`. Source repo `rathena` (`$RATHENA_PATH`), commit not recorded. No proxy captures exist in this workspace (`tools/proxy/packet_captures/` is absent), so everything below is from source, not wire-verified. When a capture disagrees, trust the capture.

Related: command interception of this packet is in [mechanics/atcommands.md](../mechanics/atcommands.md).

## 0x00f3 CZ_REQUEST_CHAT (client to map, public chat / command carrier)

> Open findings: [PF-06](../../../PACKET_FINDINGS.md#pf-06).

* **Opcode / length:** `0x00f3`, dynamic (`-1`). Handler `clif_parse_GlobalMessage`, offsets `pos0=2` (u16 packet length), `pos1=4` (text start). Source: `src/map/clif_packetdb.hpp:497` (`parseable_packet(0x00f3,-1,clif_parse_GlobalMessage,2,4)`, active for `PACKETVER >= 20050110`). `src/map/clif_shuffle.hpp` has no chat entry for the `> 20180307` branch (the one that applies to 20260219), so there is no override. NOTE: `0x00f3` is reused for other handlers in older `#if` blocks of `clif_packetdb.hpp` (`MoveToKafra` line 95, `ChangeDir` 358-463) but line 497 is the last definition and wins.
* **Note on the shuffle table:** the same table maps `0x0360 -> TickSend`, `0x0361 -> ChangeDir` etc. (see [map.md](./map.md)); `0x00f3` is not touched.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | packet_id | `0x00f3` |
| 2 | 2 | u16 | packet_length | Total frame length including the 4-byte header |
| 4 | N | bytes | text | `"<CharName> : <message>"`, see below |

### Text format and validation (`clif_process_message`, `clif.cpp:10442-10560`)

* `inputLength = packet_length - 4`; `packet_length < 4` is rejected (warning, packet dropped, session kept).
* Text must begin with the speaker's own character name (`sd->status.name`, compared over its full length, max 23 chars) immediately followed by the 3-byte separator `" : "` (space, colon, space). Needs at least `nameLen + 3 + 1` bytes.
* Name/separator mismatch is treated as a hacked/desynced client: the server logs a warning and **disconnects the session** (`set_eof`). Message is not processed.
* `message = text[nameLen+3 ..]`. For `PACKETVER >= 20151001` (applies) the message is NOT NUL-terminated on the wire: the server adds one to the length and copies `messageLength` bytes then terminates. (Pre-20151001 required a terminating NUL and an exact strlen match; not applicable.)
* Length cap: `messageLength > CHAT_SIZE_MAX-1` (`CHAT_SIZE_MAX = 256`, `map.hpp:286`) is rejected, packet dropped. So at most 254 message bytes plus the implied NUL.
* If the player has a `fakename` set, the echoed name uses the fakename, but the validation above still uses the real name.
* The echo string is `"<name> : <message>"` (reconstructed with the same separator).
* Order in `clif_process_message` after building strings: (1) `is_atcommand(fd, sd, message, 1)`; if it returns true the packet is consumed and nothing is broadcast; (2) muted (`sc.cant.chat`) or `PCBLOCK_CHAT` returns silently (no chat, no error). Commands therefore still work while muted unless the SC_NOCHAT manner includes `MANNER_NOCOMMAND`.

### When chat is a command vs normal chat

The message portion (after `Name : `) goes to `is_atcommand`. It is a command only if its first byte is `@` (atcommand) or `#` (charcommand). Detailed rules, including "no permission means it is shown as normal chat", are in [mechanics/atcommands.md](../mechanics/atcommands.md#is_atcommand-flow).

### Normal-chat handling (`clif_parse_GlobalMessage`, `clif.cpp:11511-11540`)

1. If the player is bound to a channel (`gcbind`) with chat rights, the message goes to the channel instead.
2. Otherwise broadcast `0x008d` to others (target `AREA_CHAT_WOC` = everyone in the area, excluding the speaker; or `CHAT_WOS` when inside a chat room). Area box is `x +/- (AREA_SIZE-5)`, `y +/- (AREA_SIZE-5)` (`clif.cpp:527`).
3. Echo `0x008e` back to the speaker with the same `"Name : message"` text.
4. Listening NPCs (pcre `npc_chat`) are triggered; chat is logged.

## 0x008d ZC_NOTIFY_CHAT (map to client, to others in area)

* Dynamic length. Struct `PACKET_ZC_NOTIFY_CHAT` (`packets_struct.hpp:2337`); sender `clif_GlobalMessage` (`clif.cpp:6756`). `clif_packetdb.hpp:41` `packet(0x008d,-1)`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | packet_id | `0x008d` |
| 2 | 2 | u16 | packet_length | `8 + strlen(text) + 1` |
| 4 | 4 | u32 | GID | `bl.id` of the speaker (= account_id for players, see [map.md player wire identifiers](./map.md)) |
| 8 | N | bytes | message | `"Name : message"`, NUL-terminated (included in length). Truncated to `CHAT_SIZE_MAX` (256) bytes incl. NUL |

## 0x008e ZC_NOTIFY_PLAYERCHAT (map to client, to self; also the generic system-message line)

> Open findings: [PF-17](../../../PACKET_FINDINGS.md#pf-17).

* Dynamic length. Struct `PACKET_ZC_NOTIFY_PLAYERCHAT` (`packets_struct.hpp:2344`). `clif_packetdb.hpp:42` `packet(0x008e,-1)`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | packet_id | `0x008e` |
| 2 | 2 | u16 | packet_length | `4 + strlen(text) + 1` |
| 4 | N | bytes | message | NUL-terminated text |

* Used for two things: (a) the speaker's own chat echo (text `"Name : message"`), (b) **`clif_displaymessage`**, the plain white system line used by all atcommand feedback (`clif.cpp:6664-6708`). For (b) the text is split on `\n` and each non-empty line is sent as its own `0x008e` (empty lines skipped); line capped at 255 bytes; length field `5 + len` (4 header + len + NUL). The `0x02c1 ZC_NPC_CHAT` alternative in that function is compiled only for `PACKETVER == 20141022`, so it does NOT apply to 20260219.
* `clif_messagecolor` (coloured system lines, e.g. `@` feedback in colour) uses a different packet (`ZC_NPC_CHAT 0x02c1`: len u16, GID u32, colour BGR u32, text); not used by `@item`. Not verified further here.
