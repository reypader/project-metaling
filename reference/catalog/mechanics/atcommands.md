# GM Atcommands (`@item` and testing helpers)

Target: `PACKETVER = 20260219`, renewal (`db/re`). Source: `rathena` (`$RATHENA_PATH`), primary `src/map/atcommand.cpp`, `conf/groups.yml`, `conf/atcommands.yml`, `conf/msg_conf/map_msg.conf`. rust-ro (MIT) has a tiny implementation (see end). Chat packet details: [packets/chat.md](../packets/chat.md). Inventory add semantics (`pc_additem`, inventory packets): see [inventory.md](./inventory.md#3-pc_additem); packets in [packets/inventory.md](../packets/inventory.md).

## is_atcommand flow

`is_atcommand(fd, sd, message, type)` at `atcommand.cpp:11941-12108`. `type`: 0 script, 1 player-typed chat (the chat path), 2 console, 3 script `useatcmd`. Returns true = consumed (do not chat), false = treat as normal chat.

1. Empty message returns false. Symbols: `atcommand_symbol = '@'`, `charcommand_symbol = '#'` (`atcommand.cpp:89-90`). A leading `|xx` (3-byte langtype codepage indicator) is skipped if followed by a symbol.
2. First char not `@`/`#` returns false (normal chat).
3. Type 1/3 only: if map has `nocommand` mapflag and `group level < flag value`, shows msg 143 "Commands are disabled on this map." and returns false.
4. `#` charcommand: syntax `#cmd "<char name>" <params>` or `#cmd <charname> <params>`; target looked up with `map_nick2sd`; not found shows msg 1389 `"%s failed. Player not found."`. Bad syntax: level-0 players with no right to the command get false (normal chat), others get msg 1388. The command then runs against the target's session.
5. Split into `command` (including symbol) and `params` (rest of line after first whitespace, up to newline) with `"%255s %255[^\n]"`. Params are the raw remainder string; each command parses its own.
6. Alias resolution: `atcommand_alias_db.checkAlias(command+1)` (symbol stripped, case-insensitive name table loaded from `conf/atcommands.yml` `Aliases:` + `conf/import/atcommands.yml`), then lookup `get_atcommandinfo_byname`.
7. Unknown command: group level 0 returns false (looks like normal chat, silently); level > 0 shows msg 153 `"%s is Unknown Command."` plus suggestions, returns true.
8. Permission (type 1 only): `info->at_groups[group_index] == 0` (or `char_groups` for `#`) returns **false: the text is sent as ordinary public chat, no error message**. Dead players with permission `disable_cmd_dead` get msg 1393 and the command is consumed.
9. Run `info->func(fd, ssd, command, params)`. Non-zero return: msg 154 `"%s failed."` (with the typed command incl. symbol) via displaymessage, returns true. Zero: logged via `log_atcommand` (only if the group has `LogCommands: true`; Support and above in default groups.yml) and returns true. Note: `@item` returns 0 even when `pc_additem` fails (see below), so the "failed" message is not shown for inventory-full cases.
10. Also: `idletime` is reset when configured; `PCBLOCK_COMMANDS` blocks with msg 154; NPC-talk state can disable commands (`disable_atcommand_on_npc`).

### Permissions (player group)

* `login.group_id` (tinyint, default 0; `sql-files/main.sql:776`) is loaded to `sd->group_id`; `pc_group_pc_load` (`pc_groups.cpp:348`) resolves it in `player_group_db` from `conf/groups.yml` and **kicks** the player on an unknown group id.
* A group can use a command when it has `all_commands` permission (`PC_PERM_USE_ALL_COMMANDS`, `pc_groups.cpp:325`) or the lowercased command name is in its `Commands:` list (own or inherited); `CharCommands:` for `#`. Aliases resolve to the parent command name before the check.
* Default `groups.yml` (Version 1): Id 0 Player (L0, only `changedress`, `resurrect`), 1 Super Player, 2 Support (L1), 3 Script Manager, 4 **Event Manager (L1) has `item`, `zeny`, `monster`, `killmonster2`**, 5 VIP, 10 Law Enforcement (L2), 99 Admin (L99, `all_commands: true`). `killmonster` (with drops), `heal`, `lvup`, `go` etc.: `go` is in Super Player; the rest are in higher groups or Admin; set test accounts to `group_id = 99` to get everything. Our server could also use its own simpler gate.
* Per-command levels used elsewhere: `pc_get_group_level` (e.g. nocommand mapflag).

## @item

Handler `ACMD_FUNC(item)` `atcommand.cpp:1462-1547`. Same handler serves `@itembound`.

* **Syntax:** `@item <name|id>[:<name|id>...] [amount]`; name may be quoted `"Red Potion"`. `@itembound <item> <amount> <bound>` requires all three (bound 1 Account, 2 Guild, 3 Party, 4 Character; msg 298 on invalid; `BOUND_*` in `mmo.hpp:1124`). `conf/atcommands.yml:622` help text. Several items can be given joined by `:`, all get the same amount.
* **Parse:** `sscanf` quoted form first (`"\"%99[^\"]\" %11d"`), else unquoted `%99s %11d`; the name buffer is 100 bytes; only the first whitespace-delimited token is the name when unquoted, so display names with spaces MUST be quoted. Missing/unparseable: msg 983 `"Please enter an item name or ID (usage: @item <item name/ID> <quantity>)."`, returns -1 (then msg 154 `"@item failed."`).
* **Lookup order** (per `:`-separated token): `item_db.searchname(token)` first, which tests the lowercased **AegisName** map, then the lowercased display **Name** map (`itemdb.cpp:1262`); only if both miss is the token parsed with `strtoul(token,10)` and looked up as numeric **id**. Consequence: a purely numeric string is an id unless an item is literally named that. Not found: msg 19 `"Invalid item ID or name."`, returns -1.
* **Amount:** omitted or `<= 0` becomes 1. No upper clamp in the command; `pc_additem` rejects `amount > MAX_AMOUNT (30000)` (`mmo.hpp:81`) with `ADDITEM_OVERAMOUNT`. `%11d` caps digits.
* **Non-stackables** (equipment, pet eggs etc., `!itemdb_isstackable2`): per-call count is 1 and the loop runs `amount` times, adding one at a time (each its own inventory slot / own packet). Stackables: one `pc_additem` of the full amount.
* **Pet eggs:** `pet_create_egg(sd, id)` is tried first for each unit; if the item is a pet egg it creates a pet entry instead and `pc_additem` is skipped for that unit.
* **Item struct:** only `nameid`, `identify = 1` (identified), `bound` (0 for `@item`) set; refine 0, no cards, no attribute. Log type `LOG_TYPE_COMMAND`.
* **Failure handling:** `pc_additem` returns `e_additem_result` (`pc.hpp:113`): 0 SUCCESS, 1 INVALID, 2 OVERWEIGHT, 3 ITEM (cannot carry that item type, e.g. unique conflict), 4 OVERITEM (no free slot), 5 OVERAMOUNT, 6 REFUSED_TIME, 7 STACKLIMIT. A non-zero value is stored in `flag` and sent to the client with `clif_additem(sd, 0, 0, flag)`, i.e. the item-pickup ack with `result = flag` (client shows its own "cannot carry"/"too heavy" text). Command still returns 0. `flag` holds only the result of the LAST `pc_additem` call, so earlier failures in a loop are masked by a later success (and no "Item created." if the last one failed).
* **Success:** `pc_additem` itself sends the inventory update (per added item, `clif_additem(sd, idx, amount, 0)`), then `@item` shows msg 18 `"Item created."` via `clif_displaymessage` = `0x008e` (see chat.md). Only shown if the final `flag == 0`.
* **Pickup ack packet for 20260219:** `PACKET_ZC_ITEM_PICKUP_ACK` header is `0x0b41` (`packets_struct.hpp:581`, since `PACKETVER_MAIN_NUM >= 20200916 || PACKETVER_RE_NUM >= 20200723`). Field order (`packets_struct.hpp:540-577`): idx u16 (`Index`), count u16, nameid u32, identified u8, damaged u8, slot (`EQUIPSLOTINFO`, 4x u16 cards), location u32, type u8, result u8, HireExpireDate i32, bindOnEquipType u16, 5x item-option triples (id u16, value i16, param u8 = 25 bytes), favorite u8, look u16, refiningLevel u8, grade u8. Check exact total against the compiled struct before relying on it; `clif_additem` on failure sends a zeroed struct with only header, `Index = client_index(0)`, `count = 0`, `result = flag`. Verify the wire length with a capture.
* **Messages (`conf/msg_conf/map_msg.conf`):** 18 "Item created.", 19 "Invalid item ID or name.", 154 "%s failed.", 983 usage (@item), 295/298 itembound usage / bound type. All shown with `clif_displaymessage`.

## @item2

`ACMD_FUNC(item2)` `atcommand.cpp:1549-1625`. All params mandatory: `@item2 <name|id> <amount> <identify_flag> <refine> <attribute> <card1> <card2> <card3> <card4>` (`@itembound2` adds `<bound_type>`; usage msgs 984/985 for @item2, 296/297/298 for the bound variant). Single item only (no `:` list); quoted name first, then lookup same as @item (searchname then numeric id; `searchname` is called with the raw `item_name`). For equipment: `refine` clamped to `[0, MAX_REFINE]`; for non-equipment, `identify = 1` and `refine = attribute = 0` forced. `attribute` is the broken flag. Cards are written verbatim to `card[0..3]`. Same non-stackable loop (one at a time), pet egg, `pc_additem`, `clif_additem` failure, and msg 18/19 handling as @item. Cards are NOT validated against the card DB.

## Other testing-friendly commands (names from `conf/atcommands.yml`; effect from help text)

| Command (aliases) | Syntax | Effect |
| :--- | :--- | :--- |
| `@monster` (`@spawn`, `@summon`; `@monster2`) | `<name\|id> [count [name [x [y]]]]` | Spawn monsters near you / at coordinates. Event Manager and up |
| `@killmonster` | `<map>` | Kill all monsters on the map (they drop) |
| `@killmonster2` | none | Kill all monsters on your map without drops |
| `@baselevelup` (`@blvl`, `@lvup`, `@baselevel`, `@baselvl`, `@baselvup`, `@baselvlup`, `@blevel`) | `<levels>` (negative lowers) | Change base level |
| `@joblevelup` (`@jlvl`, `@jlevel`, `@joblevel`, `@joblvl`, `@joblvlup`, `@joblvup`) | `<levels>` | Change job level |
| `@zeny` | `<amount>` (negative removes; bounded by MAX_ZENY = INT_MAX) | Give zeny |
| `@heal` | `[hp sp]` | Heal given amounts; no params = full heal |
| `@str @agi @vit @int @dex @luk` | `<amount>` | Raise (or lower with negative) that stat by amount (stat points not consumed) |
| `@stat_all` (`@allstats`, `@allstat`, `@statall`, `@statsall`) | `[amount]` | Raise all six stats; no/zero param sets all to max |
| `@statuspoint` (`@stpoint`) | `<n>` | Give stat points |
| `@skillpoint` (`@skpoint`) | `<n>` | Give skill points |
| `@mapmove` (`@rura`, `@warp`) | `<map> [x y]` | Warp self. `CZ_MOVETO_MAP 0x0140` (/mm) is turned into `@mapmove map x y` and run via `is_atcommand` |
| `@go` | `<city name\|number>` | Warp to preset city (0 prontera, 1 morocc, 2 geffen, 3 payon, 4 alberta, 5 izlude, ...); -1..-3 memo points; 15 start point. In Super Player group |
| `@jumpto` (`@goto`, `@warpto`) | `<char name>` | Warp to a player |
| `@load` (`@return`) | none | Warp to save point |
| `@alive` | none | Revive self |
| `@speed` | `<1-1000>` | Walk delay; 1 fastest, default 150 |
| `@hide` | none | Toggle GM invisibility |
| `@refresh` | none | Resend position/state to client |
| `@delitem` | `<name\|id> <amount>` | Remove items from inventory |

Exact semantics, bounds, and messages for these are not extracted yet (names/syntax only).

## rust-ro (MIT, secondary) 

`rust-ro/server/src/server/request_handler/chat.rs` only treats a message as a command if it starts with `"<CharName> : @"` (fixed symbol, TODO configurable) and dispatches to `atcommand.rs`, which has `item`, `go`, `warp`, `blvl`, `jlvl`, `heal`, `speed`, `reload`, etc. (line 61 `"item" =>`, `handle_item` line 202). No group/permission check was observed. Useful only as a feature checklist.
