# Packet Findings

Open questions and disagreements about wire packets for `PACKETVER = 20260219`. Each one comes from reading rAthena source, our catalog (`reference/catalog/`) and our code side by side. Most were written from source before any capture existed (2026-10-23). The first real-client captures (the warp captures, 2026-10-23) add evidence where they touch an entry.

### How to use this file

- Each finding has a permanent id (`PF-NN`) and an anchor (`#pf-nn`). Link to the anchor from code comments, specs and catalog notes. Never reuse an id.
- **Evidence** records what was already checked, so nobody has to dig through the same source again. Start from it and only do the step under **Resolve by**.
- A capture beats rAthena source when they disagree (CLAUDE.md).
- When a finding is resolved: set **Status** to `Resolved YYYY-MM-DD`, add a line saying what settled it (capture file, test), fix the catalog and code it names, and move its summary row to the Resolved table. Keep the entry text.
- rAthena paths are relative to `$RATHENA_PATH/src` unless noted, at the checkout used on 2026-10-23 (commit `d4b8e7b8f`). Line numbers drift; function names don't.

## Summary

| Id | Topic | Kind | Status |
| :--- | :--- | :--- | :--- |
| [PF-03](#pf-03) | `0x009d` ZC_ITEM_ENTRY rejected as legacy, but needed (19) | Code disagrees | Open; table row added from source (19), capture still wanted |
| [PF-05](#pf-05) | Which client opcodes apply: shuffle block vs packetdb | Source conflict | Resolved from source; capture still wanted |
| [PF-07](#pf-07) | `0x0b41` ZC_ITEM_PICKUP_ACK length 70 and the failure form | Struct only | Open |
| [PF-08](#pf-08) | Inventory list and item ack layouts | Struct only | Open |
| [PF-09](#pf-09) | Exp and level-up packets and their order | Struct only | Open |
| [PF-10](#pf-10) | Status window packets (`0x00bd`, `0x0141`, `0x00be`, `0x00bc`) | Struct only | Open |
| [PF-11](#pf-11) | Equip packets (`0x0998`, `0x0999`, `0x099a`) | Struct only | Open |
| [PF-12](#pf-12) | Where HP, SP, zeny and levels come from at login | Behaviour | Open |
| [PF-13](#pf-13) | Attack request and swing packet order | Behaviour | Open |
| [PF-14](#pf-14) | Attack range: straight line reaches one cell further | Behaviour | Open |
| [PF-15](#pf-15) | Mob kill timing: drops at +0 ms, vanish at +250 ms | Behaviour | Open |
| [PF-16](#pf-16) | ASPD value on the wire is amotion | Behaviour | Open |
| [PF-17](#pf-17) | `0x02c1` coloured message layout | Minor | Open |
| [PF-18](#pf-18) | `MSI_WRONG_BULLET` is 2493 or 2494 | Minor | Open |
| [PF-19](#pf-19) | Server rules not settled by source (overweight attack, DEF clamps, regen weight rate) | Server rule | Open |
| [PF-20](#pf-20) | Login-time `0x0091` that metaling doesn't send | Behaviour | Open |
| [PF-21](#pf-21) | Packets the proxy can't frame (backlog: missing/wrong `table.rs` lengths; a misframe can stall the proxy log) | Code gap | Open; item rows fixed |

### Resolved

| Id | Topic | Resolved | By |
| :--- | :--- | :--- | :--- |
| [PF-01](#pf-01) | `0x01d7` ZC_SPRITE_CHANGE2: table says 11, source sends 15 | 2026-10-23 | `item_drop_pickup_sequence.txt`; `table.rs` fixed, test `pkt_item_01_table_lengths` |
| [PF-02](#pf-02) | `0x0add` ZC_ITEM_FALL_ENTRY: table says 22, source sends 24 | 2026-10-23 | `item_drop_pickup_sequence.txt`; `table.rs` fixed, test `pkt_item_01_table_lengths` |
| [PF-04](#pf-04) | `0x00a1` ZC_ITEM_DISAPPEAR missing from table (6) | 2026-10-23 | `item_drop_pickup_sequence.txt`; `table.rs` fixed, test `pkt_item_01_table_lengths` |
| [PF-06](#pf-06) | Chat opcode (`0x00f3`, not `0x008c`) and trailing NUL | 2026-10-23 | `packet_sequence-20261023_030246.txt`; decoder test `pkt_chat_01_decodes_global_message` |

## Code disagreements

These are places where `crates/ro-packets/src/table.rs` disagrees with what rAthena sends. rAthena frames server-to-client packets by `sizeof(struct)`, not by its own `clif_packetdb.hpp` lengths, so stale packetdb lengths don't break rAthena. They do break us anywhere our length table frames server packets (the proxy, client-side tests).

### PF-01

**`0x01d7` ZC_SPRITE_CHANGE2 length**

- **Status:** Resolved 2026-10-23. Settled by capture (15); `table.rs` fixed in milestone M1 of [`specs/items/plan.md`](specs/items/plan.md), pinned by `pkt_item_01_table_lengths`.
- **Also in** `item_drop_pickup_sequence.txt` (2026-10-23): every `0x007d` gets `0x01d7` misframed the same way, with the leftover `00 00 00 00` merged into the following `0x0b08` (`08 0b 06 00 00 00`).
- **Capture:** the warp captures ([`reference/catalog/captures/`](reference/catalog/captures/README.md) `packet_sequence-20261023_030246.txt`, `_031902.txt`, `_032015.txt`) send `0x01d7` at every `LoadEndAck`. The proxy framed it at 11 and then logged the remaining 4 bytes as a bogus `0x0000` frame, alone or with the following `0x0b08`. That gives `81 84 1e 00 | 02 | b1 04 00 00 | 00 00 00 00` = AID, LOOK_WEAPON, 1201, 0: 15 bytes. Observers in view receive it as well as self.
- **Ours:** `crates/ro-packets/src/table.rs:235` has `0x01d7 => Fixed(11)`.
- **Source:** 15 bytes: `u16 op, u32 AID, u8 type, u32 val, u32 val2`. The layout is `packets_struct.hpp:2591-2603` for `PACKETVER_MAIN_NUM >= 20181121`, which applies here. It is sent from `clif_sprite_change` (`clif.cpp:3935-3960`). The 11 comes from the stale `packet(0x01d7,11)` at `clif_packetdb.hpp:226`, the old u16/u16 layout.
- **Resolve by:** a capture of equipping a weapon or headgear. Check the frame length of `0x01d7`.
- **Catalog:** [packets/equipment.md#0x01d7-zc_sprite_change2-look-change](reference/catalog/packets/equipment.md#0x01d7-zc_sprite_change2-look-change).

### PF-02

**`0x0add` ZC_ITEM_FALL_ENTRY length**

- **Status:** Resolved 2026-10-23. Settled by capture (24); `table.rs` fixed in milestone M1 of [`specs/items/plan.md`](specs/items/plan.md), pinned by `pkt_item_01_table_lengths`.
- **Capture:** [`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt) lines 1116-1117 and 1123-1124. A Poring death sends `0x0add` to both clients. The proxy frames it at 22 and logs the last 2 bytes (`00 00`) as a bogus 2-byte `0x0000` frame. The full payload is `41 00 00 00 | 8d 03 00 00 | 03 00 | 01 | 9d 00 | 4b 01 | 09 | 09 | 01 00 | 00 | 00 00`: floor id 0x41, ITID 909 (Jellopy), type 3 (Etc), identified 1, x 157, y 331 (the mob's own cell, `DIR_CENTER`), subX 9, subY 9, count 1, showdropeffect 0, dropeffectmode 0. That's 24 bytes.
- **Ours:** `crates/ro-packets/src/table.rs:956` has `0x0add => Fixed(22)`.
- **Source:** 24 bytes. `struct packet_dropflooritem` (`packets_struct.hpp:597-618`) has `u32 ITID` from `PACKETVER >= 20181121`, and `clif_dropflooritem` (`clif.cpp:865`) sends `sizeof(p)`. The 22 at `clif_packetdb.hpp:1921` dates from 2018-04-18, when ITID was u16.
- **Resolve by:** a capture of killing a mob until it drops an item. Check the `0x0add` frame length.
- **Catalog:** [packets/ground_items.md#0x0add-zc_item_fall_entry](reference/catalog/packets/ground_items.md#0x0add-zc_item_fall_entry).

### PF-03

**`0x009d` ZC_ITEM_ENTRY is rejected as legacy**

- **Status:** Open. `item_drop_pickup_sequence.txt` doesn't cover it: the item was picked up while still in view. Milestone M1 of [`specs/items/plan.md`](specs/items/plan.md) added the row at 19 from source and removed it from the legacy list, since ground items need it.
- **Ours:** `crates/ro-packets/src/table.rs:1206` lists `0x009d` in the "legacy opcodes rejected" test.
- **Source:** still used at this packetver. `clif_getareachar_item` (`clif.cpp:5391`) sends it, 19 bytes by struct, to a player who walks into view of an item already on the ground. `0x0add` is only for the moment of the drop. There is no `clif_packetdb.hpp` entry; the length comes from the struct only.
- **Resolve by:** a capture of walking up to an item that was dropped earlier. Then add the row and drop it from the legacy list.
- **Catalog:** [packets/ground_items.md#0x009d-zc_item_entry-already-on-the-ground](reference/catalog/packets/ground_items.md#0x009d-zc_item_entry-already-on-the-ground).

### PF-04

**`0x00a1` ZC_ITEM_DISAPPEAR has no table row**

- **Status:** Resolved 2026-10-23. Settled by capture (6); `table.rs` fixed in milestone M1 of [`specs/items/plan.md`](specs/items/plan.md), pinned by `pkt_item_01_table_lengths`.
- **Capture:** [`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt) lines 1134-1136 and 1139-1141. After the pickup both clients get `a1 00 | 41 00 00 00`. The proxy logs it as the fragments `0x00a1`(1), `0x4100`(1), `0x0041`(4): 6 bytes. It is sent to `AREA`, so the picker and the observer both get it.
- **Ours:** no row in `crates/ro-packets/src/table.rs`.
- **Source:** 6 bytes (`u16 op, u32 floor item id`), sent by `clif_clearflooritem` (`clif.cpp:930`) on pickup and expiry. No `clif_packetdb.hpp` entry; struct size only.
- **Resolve by:** the same capture as PF-02 plus picking the item up.
- **Catalog:** [packets/ground_items.md#0x00a1-zc_item_disappear](reference/catalog/packets/ground_items.md#0x00a1-zc_item_disappear).

## Opcode choice

### PF-05

**Client opcodes come from the `PACKETVER > 20180307` shuffle block**

- **Status:** Resolved from source on 2026-10-23. A capture should still confirm it.
- **Conflict:** some catalog notes said "no `clif_shuffle.hpp` block for 20260219" (they searched for `== 20260219`) and fell back to `clif_packetdb.hpp`. There, later blocks re-register `0x0362`, `0x0363`, `0x035f`, `0x0436` and others as unrelated handlers.
- **Evidence:**
  - `clif.cpp:25843-25844` (`packetdb_readdb`) includes `clif_packetdb.hpp` and then `clif_shuffle.hpp`. `packetdb_addpacket` (`clif.cpp:25808-25830`) overwrites one table, so for any opcode the shuffle block lists, the shuffle entry wins.
  - `clif_shuffle.hpp` is a chain of `#elif PACKETVER == <date>` ending at `:4692` (`== 20180307`), followed by `:4723 #elif PACKETVER > 20180307`. That last branch (`:4723-4761`) applies to 20260219. There are no outer guards.
  - `config/packets.hpp:19-24` defines `PACKETVER_RE` only for `(>20151104 && <20180704) || (>=20200902 && <=20211118)`. So for 20260219, `PACKETVER_MAIN_NUM = 20260219` (`:28`) and `PACKETVER_RE_NUM` is undefined. Every `PACKETVER_MAIN_NUM >= X` branch is taken.
  - Final opcodes from the shuffle block: `0x035f` walk (5), `0x0360` tick (6), `0x0361` direction (5), `0x0362` pickup (6), `0x0363` drop (6), `0x0368` reqname (6), `0x0369` solve char name (6), `0x0436` enter (23, offsets 2/6/10/14/22), `0x0437` action (7), `0x0438` skill on target (10), `0x0366` skill on position (10), `0x0367` (31).
  - Not in the shuffle block, so the packetdb base entry stands: `0x0439` use item (8), `0x0998` equip (8, from `HEADER_CZ_REQ_WEAR_EQUIP`, `packets.hpp:1503-1509`), `0x00ab` unequip (4), `0x00bb` raise stat (5), `0x0118` cancel lock-on (2).
  - All of these match `crates/ro-packets/src/table.rs` and the catalog.
  - Opcodes the shuffle block doesn't list keep their last `clif_packetdb.hpp` registration (cumulative `#if PACKETVER >= date` blocks, last one wins). Stale entries such as `0x0072` enter and `0x0085` walk stay in the table but the client doesn't send them.
- **Left to do:** a capture of login, walking, attacking, pickup, drop, use, equip and stat raise. Also fix the policy text in [packets/map.md](reference/catalog/packets/map.md#packet-shuffling-precedence-and-macro-offset-rules), which states the rule but not the branch.
- **Catalog:** [packets/inventory.md#client-to-server-opcodes](reference/catalog/packets/inventory.md#client-to-server-opcodes), [packets/equipment.md](reference/catalog/packets/equipment.md).

### PF-06

**Chat opcode and trailing NUL**

- **Status:** Resolved 2026-10-23. Settled by capture: opcode `0x00f3`, no trailing NUL. Decoded in milestone M4 of [`specs/items/plan.md`](specs/items/plan.md) (`pkt_chat_01_decodes_global_message`).
- **Capture:** [`packet_sequence-20261023_030246.txt`](reference/catalog/captures/packet_sequence-20261023_030246.txt) has `0x00f3` with length 26 and payload `testtest : @item knife` (22 bytes, no `00`). [`packet_sequence-20261023_031902.txt`](reference/catalog/captures/packet_sequence-20261023_031902.txt) has length 25, `testtest : @goto test`.
  - The server appends the NUL itself (`clif.cpp:10516`).
  - A name or `" : "` mismatch disconnects the session (`set_eof`, `clif.cpp:10501`).
  - `is_atcommand` sees the text after the prefix.
  - Details: [packets/chat.md](reference/catalog/packets/chat.md). Resolved: `specs/items` M4 decodes it.
- **Conflict:** the unconditional base section of `clif_packetdb.hpp` registers `0x008c` as GlobalMessage (`:40`) and `0x00f3` as MoveToKafra (`:95`). That made `0x008c` look like the chat opcode.
- **Evidence:**
  - `clif_packetdb.hpp` blocks are cumulative `#if PACKETVER >= date`, and the last registration wins.
  - `0x00f3` is registered as GlobalMessage (`-1`, offsets 2/4) at `:305` (`>= 20040726`) and again at `:497` (`>= 20050110`, block `:486-504`). Nothing later in `clif_packetdb.hpp` or `clif_shuffle.hpp` registers `0x00f3`, so `:95` is overwritten.
  - `0x008c` is last registered at `:1202` as GetCharNameRequest (14 bytes, `>= 20080827`), so the `:40` chat entry is overwritten too.
  - `0x00f3` is the only opcode that reaches `clif_parse_GlobalMessage` for this build.
- **Still open:** for `PACKETVER >= 20151001` the client sends the message without a trailing NUL. The cap is 255 bytes plus the implied NUL (`CHAT_SIZE_MAX` 256).
- **Resolve by:** a capture of typing a normal line and `@item 501`. Confirm `0x00f3` and check whether the text ends in `\0`.
- **Catalog:** [packets/chat.md#0x00f3-cz_request_chat-client-to-map-public-chat--command-carrier](reference/catalog/packets/chat.md#0x00f3-cz_request_chat-client-to-map-public-chat--command-carrier).

## Layouts known only from structs

rAthena sends these by `sizeof(struct)` and the layouts below are what the struct says at this packetver. A capture can confirm each one in a single pass.

### PF-07

**`0x0b41` ZC_ITEM_PICKUP_ACK length and failure form**

- **Status:** Length and success form settled by capture on 2026-10-23. The failure form is still open.
- **Capture:** [`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt) line 1131 (proxy-truncated to 32 payload bytes). A successful Jellopy pickup gives a 70-byte frame, with nothing misframed after it. The payload starts `05 00 | 01 00 | 8d 03 00 00 | 01 | 00 | 00×16 cards | 00 00 00 00 location | 03 type | 00 result ...`: wire index 5, count 1, nameid 909, identified 1, damaged 0, type 3, result 0. Picker-side order: `0x0b41`, then `0x00b0` SP_WEIGHT (`18 00 50 05 00 00` = 1360), then `0x08c8` type 1 (`AREA`), then `0x00a1` (`AREA`).
- **Source:** 70 bytes, opcode `0x0b41` for `PACKETVER_MAIN_NUM >= 20200916` (`packets_struct.hpp:540-595`), built by `clif_additem` (`clif.cpp:2838-2891`). The same packet reports success and failure. `result` is the `e_additem_result` number: 0 ok, 1 invalid, 2 overweight, 4 no free slot, 5 over max stack, 6 pickup refused, 7 stack limit.
- **Failure form:** only `index` (= 2, server slot 0), `count` (= 0) and `result` are non-zero. A source comment at `clif.cpp:12034` says the client needs this to unblock further pickups.
- **Resolve by:** a capture of `@item` (success), picking up with a full inventory or while overweight (failure), and picking up another player's drop too early (result 6).
- **Catalog:** [packets/inventory.md#0x0b41-zc_item_pickup_ack-add-item--pickup-result](reference/catalog/packets/inventory.md#0x0b41-zc_item_pickup_ack-add-item--pickup-result).

### PF-08

**Inventory list and item ack layouts**

- **Status:** Inventory list lengths settled by capture on 2026-10-23. The drop ack, delete, use ack and ammo packets are still open.
- **Capture:** [`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt) lines 75-79 and 200-204. After every `0x007d` the order is: `0x01d7` (15), `0x0b08` (6: `06 00 | 00 | 00`), `0x0b09` (39 = 5 + 34×1 and 311 = 5 + 34×9), `0x0b39` (277 = 5 + 68×4 and 209 = 5 + 68×3), `0x0b0b` (4: `00 00`), `0x0a9b` (4), `0x00b0` SP_WEIGHT, `0x00b0` SP_MAXWEIGHT. The whole list is re-sent on every `0x007d`, including repeats. SP_MAXWEIGHT was 20300 for a character with STR 1 and 23300 for one with STR 11, which matches `20000 + 300 × STR` for a novice. The client's `0x0363` drop (line 1151) has no visible reply: the proxy log of server→client traffic stops at 09:09:24.041 (see [PF-21](#pf-21)).
- **Source:**
  - Login list: `0x0b08` start (6 bytes for the plain inventory), `0x0b09` stackables (5 + 34 per entry), `0x013c` only if ammo is worn, `0x0b39` equipment (5 + 68 per entry), `0x0b0b` end (4).
  - In the entries, `RefiningLevel` and `grade` come after the 5-entry option array, and identified/damaged/favorite share one `Flag` byte.
  - Drop ack `0x00af` (6, count 0 = refused). Delete `0x07fa` (8, reasons 0-7). Use ack `0x01c8` (15), sent to `AREA` on success and to self on failure.
- **Resolve by:** a capture of login with at least one stackable, one equipment piece and worn ammo, then a drop and a potion use.
- **Catalog:** [packets/inventory.md](reference/catalog/packets/inventory.md).

### PF-09

**Exp and level-up packets and their order**

- **Status:** Open.
- **Source:**
  - `0x0acc` ZC_NOTIFY_EXP2, 18 bytes: `u16 op, u32 AID, i64 amount, u16 var (1 base, 2 job), u16 exp_type (1 only for quest base exp)`. Sent to self.
  - `0x0acb` ZC_LONGLONGPAR_CHANGE, 12 bytes: `u16 op, u16 var, i64 value`, for BASEEXP 1, JOBEXP 2, NEXTBASEEXP 22, NEXTJOBEXP 23.
  - `0x019b` ZC_NOTIFY_EFFECT, 10 bytes, effect 0 = base level up, 1 = job level up, to `AREA` including the leveler.
  - Base level-up order: STATUSPOINT, TRAITPOINT, BASELEVEL, BASEEXP, NEXTBASEEXP, then the stat recalc push, then the full heal, then `0x019b`.
- **Unknown:** which HP/SP `0x00b0` updates the heal produces (from `status_calc_pc` and `status_percent_heal`, not traced).
- **Resolve by:** a capture of killing a mob for exp, then a kill that levels up.
- **Catalog:** [mechanics/experience.md#wire-packets](reference/catalog/mechanics/experience.md#wire-packets), [mechanics/experience.md#level-up](reference/catalog/mechanics/experience.md#level-up).

### PF-10

**Status window packets**

- **Status:** Open.
- **Source:**
  - `0x00bd` ZC_STATUS, 44 bytes, sent only by `clif_initialstatus`. Stats are allocated points only, capped at 255. The "need" bytes are the cost of the next point. In renewal the DEF pair is soft (left) and hard (right), and the two MATK fields are swapped relative to their names.
  - `0x0141` ZC_COUPLESTATUS, 14 bytes (`u32 var, i32 base, i32 bonus`) for STR..LUK and POW..CRT.
  - `0x00be`, 5 bytes (`u16 var, u8 value`), for the stat costs.
  - Stat raise: `0x00bb` request (5) and `0x00bc` ack (6). On success the order is: `0x0141` diff pushes, `0x00be` new cost, `0x00b0` STATUSPOINT, then `0x00bc`.
  - Trait stats (POW..CRT, AP and so on) go out at login for every class.
- **Resolve by:** a capture of login, then raising STR once.
- **Catalog:** [packets/status.md](reference/catalog/packets/status.md).

### PF-11

**Equip packets**

- **Status:** Open.
- **Source:**
  - `0x0998` request, 8 bytes (`u16 index` = slot + 2, `u32 location`).
  - `0x0999` ack, 11 bytes (`index`, `u32 location`, `u16 view`, `u8 result`: 0 ok, 1 level too low, 2 fail).
  - `0x00ab` take off (4) and `0x099a` ack (9, flag 0 ok, 1 fail).
  - A displaced item gets its own unequip ack before the new item's equip ack.
  - Ammo gets `0x013c` then `0x013b` type 3, and no `0x0999`.
- **Resolve by:** a capture of equipping a weapon, swapping it for another, and equipping arrows.
- **Catalog:** [packets/equipment.md](reference/catalog/packets/equipment.md), [mechanics/equipment.md#3-pc_equipitem-flow](reference/catalog/mechanics/equipment.md#3-pc_equipitem-flow).

## Behaviour and order

### PF-12

**Where HP, SP, zeny and levels come from at login**

- **Status:** Open.
- **Source:**
  - `LoadEndAck` always sends WEIGHT and MAXWEIGHT.
  - On first login it also sends the four exp vars, SKILLPOINT and `clif_initialstatus` (`0x00bd` and follow-ups). On a map change it sends only the `0x0141` stat packets.
  - It sends no HP, SP, zeny or level packets. HP, SP and the derived stats arrive from the first `status_calc_pc`, which runs when the char server delivers the inventory and diffs against a zeroed snapshot, before `0x007d`.
  - Zeny and levels may come only from the char-select data.
- **Resolve by:** a capture of a full login from char select to standing on the map. List every packet before and after `0x007d`.
- **Catalog:** [packets/status.md#where-the-status-packets-sit-in-the-map-entry-order](reference/catalog/packets/status.md#where-the-status-packets-sit-in-the-map-entry-order), [mechanics/map_entry_actions.md](reference/catalog/mechanics/map_entry_actions.md).

### PF-13

**Attack request and swing packet order**

- **Status:** Open.
- **Source:**
  - `0x0437` action 0 is a single swing and 7 is continuous. We assume a plain click sends 7.
  - Every request first stops the current attack. A bad target fails silently.
  - Out of range: the server sends `0x0139` and does not chase; the client walks and asks again.
  - Source suggests `0x0088` (stop move) goes out twice before the first `0x08c8`.
  - `0x08c8` goes to `AREA` around the target at swing time, with `src_speed` = attacker amotion and `dmg_speed` = target dmotion (0 on a miss). The HP change lands amotion ms later.
- **Unknown:** the `target_id` the client puts in `0x0437` for sit and stand.
- **Resolve by:** a capture of clicking a mob once, holding attack, clicking a mob out of range, and sitting.
- **Catalog:** [mechanics/auto_attack.md](reference/catalog/mechanics/auto_attack.md), [packets/combat.md](reference/catalog/packets/combat.md).

### PF-14

**Attack range reaches one cell further in a straight line**

- **Status:** Open.
- **Source:** player range check is `floor(sqrt(dx² + dy²) - 0.1) <= range` (`check_distance_client`, `path.cpp:485-523`). With range 1, a target 2 cells away in a straight line is in range but `(2,1)` is not. The "one cell further" reading comes from a source comment. Line of sight is checked at distance 2 and above. Mobs use square (Chebyshev) distance instead.
- **Resolve by:** a capture of attacking with bare hands from 2 cells away in a straight line. Does the client send `0x0437` without walking, and does the server's `0x08c8` follow?
- **Catalog:** [mechanics/auto_attack.md#2-range-check-semantics](reference/catalog/mechanics/auto_attack.md#2-range-check-semantics).

### PF-15

**Mob kill timing**

- **Status:** Confirmed by capture on 2026-10-23 ([`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt) lines 1106-1131).
  - At 22.271 the killer gets `0x0977` hp 0, then the exp packets (`0x0acb`, `0x0acc`), then `0x0add`. The observer's `0x0add` goes out at 22.269, so it's the same server tick.
  - `0x0080` type 1 follows at 22.517 (+246 ms).
  - The `0x0add` cell is the mob's last `0x0088` stop cell (157,331).
  - Pickup order: `0x0b41`, `0x00b0` SP_WEIGHT, `0x08c8` type 1, `0x00a1`.
  - The client sent `0x0361` change direction right before `0x0362`, and observers got the matching `0x009c`.
- **Source:**
  - On the killing blow `0x0977` carries hp 0. The mob leaves the block grid at once without a packet.
  - Drops are queued on a timer with a 0 ms delay by default and appear as `0x0add`.
  - `0x0080` type 1 (died) goes to `AREA` 250 ms after the damage lands.
  - Successful pickup: `0x0b41`, then `0x08c8` type 1, then `0x00a1`.
- **Resolve by:** a capture of killing a mob that drops, then picking up the drop. Check the order and the gaps between packets.
- **Catalog:** [mechanics/mobs.md#31-packet-order-for-one-normal-kill-single-player-loot-no-party-at-default-conf](reference/catalog/mechanics/mobs.md#31-packet-order-for-one-normal-kill-single-player-loot-no-party-at-default-conf).

### PF-16

**ASPD on the wire is amotion**

- **Status:** Open.
- **Source:** the ASPD field in `0x00bd` and `0x00b0` carries raw `amotion` in ms. The client turning that into `200 - amotion/10` for display is general client knowledge, not something rAthena source states.
- **Resolve by:** a capture of login with a known AGI/DEX. Compare the value with the status window.
- **Catalog:** [mechanics/stats.md#aspd-renewal](reference/catalog/mechanics/stats.md#aspd-renewal).

### PF-17

**`0x02c1` coloured message layout**

- **Status:** Open. Low priority: `clif_displaymessage` uses `0x008e` at this packetver, and the `0x02c1` branch is compiled only for 20141022.
- **Source:** `clif_messagecolor` appears to be `u16 len, u32 GID, u32 color, text`. Not verified.
- **Resolve by:** only needed if we send coloured messages.
- **Catalog:** [packets/chat.md](reference/catalog/packets/chat.md).

### PF-18

**`MSI_WRONG_BULLET` value**

- **Status:** Open. Low priority.
- **Source:** 2493 or 2494 depending on a packetver branch (`clif.hpp:616` and `:634`). The branch was not resolved. The `clif_msg` packet and the `EFST_CLIENT_ONLY_EQUIP_ARROW` status packet are not in the catalog yet.
- **Resolve by:** a capture of equipping the wrong ammo type for the weapon.
- **Catalog:** [mechanics/equipment.md](reference/catalog/mechanics/equipment.md).

### PF-19

**Server rules not settled by source**

- **Status:** Open. These aren't packet layouts, but they decide what we send.
- **90% overweight and normal attacks:** source blocks skills at 90%, but no server check blocking basic attacks was found. See [mechanics/inventory.md#8-weight](reference/catalog/mechanics/inventory.md#8-weight).
- **Hard DEF/MDEF clamps:** the renewal `DEFTYPE_MIN/MAX` clamps were not located. See [mechanics/stats.md#derived-stat-formulas](reference/catalog/mechanics/stats.md#derived-stat-formulas).
- **`natural_heal_weight_rate`:** 70 for renewal comes from the `#ifdef RENEWAL` default in `battle.cpp`; the line in `conf/battle/player.conf` is commented out.
- **Resolve by:** a test on real rAthena (`./run_rathena_servers.sh`): attack at 90% weight; regen at 70-89% weight.

### PF-20

**Login-time `0x0091` that metaling doesn't send**

- **Status:** Open.
- **Capture:** every fresh login in the 2026-10-23 warp captures ([`reference/catalog/captures/`](reference/catalog/captures/README.md) `packet_sequence-20261023_030246.txt`, `_031902.txt`, `_032015.txt`) has `0x0091` with the character's own map and cell after `0x02eb`, `0x0201` and the `0x008e` MOTD lines, before the client's first `0x007d`. In one login the client then sent `0x007d` twice.
- **Source:** `pc_authok` calls `clif_changemap` explicitly on a fresh login, but not on arrival from another map-server. The source comment calls it a fix for a "login-without-aura glitch". rAthena neither waits for it nor tracks it. Details: [mechanics/map_entry_actions.md#map-entry-cz_enter-clif_parse_wanttoconnection](reference/catalog/mechanics/map_entry_actions.md#map-entry-cz_enter-clif_parse_wanttoconnection).
- **Ours:** `bins/map-server/src/service.rs` `join` sends only `0x0283`, `0x0b18` and `0x02eb` before `0x007d`.
- **Resolve by:** deciding whether to send it. Compare a metaling login capture with a real client against these captures: does the character show its aura, and does the client behave the same without it? Not needed for warps: the warp feature sends `0x0091` on a map change only.

### PF-21

**Packets the proxy can't frame (backlog)**

- **Status:** Open, low priority. Revisit when nothing else is queued, or when a capture or a client bug looks odd: a misframed packet hides the packets after it.
- **Why it matters:** `tools/proxy` frames with `crates/ro-packets/src/table.rs`. A missing or wrong length makes the proxy log `UNKNOWN / UNMAPPED` frames and then garbage 1-byte frames (`0x0000`, `0x1e84`, `0x2800`, ...) until the stream re-syncs. The map-server never sends these packets today, so only captures and client-side tests are affected.
- **Seen in the 2026-10-23 warp captures** ([`reference/catalog/captures/`](reference/catalog/captures/README.md)):
  - Server to client, missing from the table: `0x0141` (stat lines), `0x013a` (attack range), `0x0b1b`, `0x0b8d`, `0x080e` (party member HP), `0x010f` (skill list), `0x02c9`, `0x0acb`, `0x0999`, `0x019b`, `0x00b4`.
  - Client to char-server, wrong length: `0x08b8` `CH_SECOND_PASSWD_ACK` is framed at 8, but the client sends 2 more bytes. The leftover bytes appear as a bogus client frame after every PIN entry.
  - Expected and not a bug: `0x8480`/`0x8481` are the raw 4-byte AccountId echo after `CH_ENTER`/`CZ_ENTER` (`bins/CLAUDE.md`), which the proxy doesn't skip.
- **The proxy log can stall** (seen in [`item_drop_pickup_sequence.txt`](reference/catalog/captures/item_drop_pickup_sequence.txt), 2026-10-23):
  - After 09:09:24.041 the log has no server→client packets for either map session, even though the clients kept walking and sending `0x0360`, which always gets a `0x007f` reply.
  - `tools/proxy/src/forwarder.rs` forwards raw bytes *before* it logs them, so the clients were not affected. Only the log was.
  - `PacketLogger::feed_and_log` (`tools/proxy/src/logger/packet_logger.rs`) waits on `Ok(None)` when a misframe lands on bytes that read as a known opcode with a large dynamic length. It then buffers silently until that length arrives, or until the 128 KiB guard clears the buffer.
  - So the reply to the client's `0x0363` drop (line 1151) was never logged. "Nothing happened" in the client most likely means rAthena refused it with `0x00af` count 0: wire index 12 ×2 was not the Jellopy, which sat at wire index 5.
  - **Fix:** the `specs/items` M1 table rows (done). A possible proxy hardening is to log a warning when a dynamic frame stays incomplete across more than N reads.
- **Item rows fixed** in milestone M1 of [`specs/items/plan.md`](specs/items/plan.md) (2026-10-23): `0x0add` 24, `0x00a1` 6, `0x009d` 19, `0x0b41` 70, `0x0b08` dynamic, `0x0b0b` 4, `0x01d7` 15, `0x00af` 6.
- **Resolve by:** for each opcode, take the length from the capture (the gap to the next real opcode) and check it against the catalog and source. Then add it to `table.rs` with a test. Re-run the proxy over a capture and expect no `UNKNOWN` frames apart from the AccountId echo.

## Capture plan

One session through `tools/proxy` against `./run_rathena_servers.sh`, using a `group_id = 99` account, covers almost everything above:

| Step | Covers |
| :--- | :--- |
| Log in from char select to standing on the map, with a stackable, an equipment piece and worn arrows in the inventory | PF-05, PF-08, PF-10, PF-12, PF-16 |
| Say a normal line, then `@item 501 5`, then `@item 1201` | PF-06, PF-07 |
| Raise STR once | PF-10 |
| Equip a dagger, swap to another dagger, equip arrows, equip the wrong ammo | PF-01, PF-11, PF-18 |
| Click a Poring once, hold attack on another, click one out of range, attack from 2 cells in a straight line | PF-13, PF-14 |
| Kill until a drop, pick it up; walk away and back to a dropped item; drop an item; use a potion | PF-02, PF-03, PF-04, PF-08, PF-15 |
| Kill until a level-up (or `@blvl 1`) | PF-09 |
| Fill the inventory or exceed weight, then try a pickup | PF-07 |
