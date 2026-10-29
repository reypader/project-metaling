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

## @baselevelup / @joblevelup

Source: rAthena `src/map/atcommand.cpp` `ACMD_FUNC(baselevelup)` 1664-1743, `ACMD_FUNC(joblevelup)` 1745-1809 (applies to 20260219; not packet-version dependent). Helpers in `src/map/pc.cpp`: `pc_baselevelchanged` 8306, `pc_maxbaselv`/`pc_maxjoblv` 8579/8599, `pc_resetstate` 9347, `pc_resetskill` 9428, `PlayerStatPointDatabase` 8735-8780. Messages from `conf/msg_conf/map_msg.conf`, shown with `clif_displaymessage` (chat-line system message). Experience-side context: [experience.md#level-up](./experience.md#level-up), [experience.md#direct-set-scriptgm](./experience.md#direct-set-scriptgm).

Argument: `atoi(message)` (non-numeric = 0). Empty or 0 -> msg 986 (base) / 987 (job) usage text, return -1, nothing changes. Max levels come from the job DB per class (`MaxBaseLevel`/`MaxJobLevel`, see [../db/exp_tables.md](../db/exp_tables.md)), not from exp tables.

### @baselevelup `<n>`

Positive n:
1. If `base_level >= max_base` -> msg 47 "Base level can't go any higher.", return -1.
2. n clamped to `max_base - base_level`.
3. Status points granted: sum over each level step L -> L+1 (L = old level .. new-1) of `max(0, Points[L+1] - Points[L])` from statpoint.yml (`pc_gets_status_point`); trait points likewise (`pc_gets_trait_point`). Added to `status_point`/`trait_point`; `base_level += n`.
4. In order: `status_calc_pc(SCO_FORCE)`; `status_percent_heal(100,100)` (full HP and SP; see heal pushes below); `clif_misceffect` effect 0 (`0x019b` ZC_NOTIFY_EFFECT, target AREA = self + neighbours); achievements; msg 21 "Base level raised."
Negative n (|n| = k):
1. If `base_level == 1` -> msg 158 "Base level can't go any lower.", return -1.
2. k clamped to `base_level - 1` (cannot reach below 1).
3. Points to remove = sum over steps (L-1 -> L) for L = old level down to old-k+1 of the same per-step delta (status and trait).
4. If `status_point < remove` OR `trait_point < remove_trait`: `pc_resetstate` is called (BEFORE base_level is decremented): status_point = `Points[old_level]` (+ `transcendent_status_points` for upper/4th-primary, trait_point also reset), all six stats set to 1 (trait stats to 0), sends all `SP_STR..`, `SP_U*` cost vars, `SP_STATUSPOINT`, `SP_TRAITPOINT`, clears TK mission, `status_calc_pc(SCO_NONE)`. Then the removal is subtracted (floored at 0 independently per pool). Net result after a reset: `Points[old_level] - remove` (roughly `Points[new_level]`), so stats are reset but the player is left with the correct total for the new level. If the player has enough unspent points, only the subtraction happens and allocated stats are untouched.
5. `base_level -= k`; msg 22 "Base level lowered."; `status_calc_pc(SCO_FORCE)`. NO heal and NO effect on level down (HP/SP only change via recalc clamping).
Common tail (both directions, only reached on success): `base_exp = 0`; then `clif_updatestatus` in order `SP_STATUSPOINT (0x00b0)`, `SP_TRAITPOINT`, `SP_BASELEVEL (0x00b0)`, `SP_BASEEXP (0x0acb)`, `SP_NEXTBASEEXP (0x0acb)`; `pc_baselevelchanged`; `party_send_levelup` if in a party; if n > 0 and battle conf `atcommand_levelup_events` -> NPC event `NPCE_BASELVUP` (OnPCBaseLvUpEvent). Exp is reset even when n is clamped.
`pc_baselevelchanged` (both directions): for every equip slot, if the item's `elvmax` (EquipLevelMax) is non-zero and `base_level > elvmax`, `pc_unequipitem(sd, idx, 3)` (flag 3 = 1 (recalc status afterwards) + 2 (force unequip, ignores restrictions such as cursed), `pc.cpp:12390-12395`; the item stays in inventory); then `pc_show_questinfo` (refresh quest-info icons for the new level). Only the MAX level is checked here; EquipLevelMin is not rechecked by this function (a min-level item stays equipped after a level drop).

### @joblevelup `<n>`

Positive n: if `job_level >= max_job` -> msg 23 "Job level can't go any higher.", -1. n clamped to `max_job - job_level`. `job_level += n`; `skill_point += n` (1 per level, no table); `clif_misceffect` effect 1 (`0x019b`, AREA); achievements; msg 24 "Job level raised."
Negative n: if `job_level == 1` -> msg 159 "Job level can't go any lower.", -1. k clamped to `job_level - 1`. `job_level -= k`. If `skill_point < k`: `pc_resetskill(sd, 0)` (flag 0: all learned skills set to level 0, skill points refunded into `skill_point`; also strips riding/falcon/wug/cart/madogear options, vaporizes homunculus, etc.; it does NOT send packets itself with flag 0, no `skill_info` block resend). Then if `skill_point` is still `< k` it is set to 0, else `skill_point -= k`. Unspent points alone are used first; learned skills are only wiped when unspent points are insufficient. msg 25 "Job level lowered."
Common tail: `job_exp = 0`; `clif_updatestatus`: `SP_JOBLEVEL`, `SP_JOBEXP (0x0acb)`, `SP_NEXTJOBEXP (0x0acb)`, `SP_SKILLPOINT`; then `status_calc_pc(SCO_FORCE)`. No heal, no party notify, no `pc_baselevelchanged`/equip check, no SP_STATUSPOINT. If n > 0 and `atcommand_levelup_events` -> `NPCE_JOBLVUP`. Note: after a skill wipe via this command rAthena never resends the skill list (`clif_skillinfoblock`), so the client list is stale until relog/refresh; a new server should resend it.

**Heal pushes after the level-up** (`status.hpp:3546-3548`, `status.cpp:1905-1960`, `status.cpp:1803-1895`, `pc.cpp:10663-10690`; applies to 20260219): `status_percent_heal(bl,100,100)` calls `status_percent_change(nullptr,bl,-100,-100,-0,flag 0)`; negative rates below -99 mean "amount = MaxHP / MaxSP" (AP rate 0 -> 0). It then calls `status_heal(bl, maxhp, maxsp, 0, 0)`, which (a) returns 0 with no pushes if the player's current HP is 0; (b) clamps each amount to the missing amount (`amount = min(amount, max - current)`); (c) returns 0 with no pushes if all of hp, sp, ap clamped amounts are 0 (and `flag&4` unset); (d) adds the amounts and calls `pc_heal(sd, hp, sp, ap, flag=0)`. In `pc_heal` with `flag&2` clear: `if (hp) clif_updatestatus(SP_HP)`, `if (sp) clif_updatestatus(SP_SP)`, `if (ap) clif_updatestatus(SP_AP)`, each on the clamped amount. So `SP_HP` is pushed only when HP was below the new MaxHP, `SP_SP` only when SP was below the new MaxSP (SP already full and MaxSP unchanged -> no `SP_SP`; a MaxSP increase from the recalc makes it non-zero -> `SP_SP` pushed). No `SP_MAXHP`/`SP_MAXSP` comes from the heal (those come from the recalc diff in `status_calc_pc`). Order in the heal: HP, SP, AP.

**Captured** ([captures/attribute_sequence.txt](../captures/attribute_sequence.txt), write-up in [packets/status.md](../packets/status.md#captured-stat-up-level-and-job-sequences)): the orders above hold on the wire. Level up: recalc diff, the heal's `SP_HP`/`SP_SP`, `0x019b`, message, then the common tail. Level down: message, clamp pushes, recalc diff, tail. Job level up: `0x019b` effect 1, message, tail, recalc diff. Job level down: message, tail, recalc diff. The `pc_resetstate` branch of `@blvl -n` and the `pc_resetskill` branch of `@jlvl -n` were not exercised. The command aliases `@blevel` and `@jlevel` work. `@blevl` is unknown and gets the unknown-command message.

**Capture 2** ([captures/attribute_sequence_2.txt](../captures/attribute_sequence_2.txt), write-up in [packets/status.md](../packets/status.md#capture-2-observer-stat-reset-failure-and-4th-class-jobs)). Applies to 20260219.

* The `pc_resetstate` branch of `@blevel -n` happened twice (`-3` at lines 464-527, `-90` at 1023-1104; zero unspent points both times) and not once (`-99`, 1314-1337, 64 unspent against 16 to remove). In the reset branch the order is: stale `0x0141` ×12 (base 1/0, bonus = old battle value - 1), `0x00be` ×12 (six stat costs, six trait costs), `SP_STATUSPOINT` = table value of the **old** level, `SP_TRAITPOINT` 0, the reset's own recalc, **then** the message, then the level change's recalc (clamp pushes, diff), then the tail with `SP_STATUSPOINT` = table value of the new level. So the message is not first when a reset happens.
* `@blevel +n` seen by a second client: `0x080e` and `0x019b` effect 0 only (lines 431, 434). The caster gets the diff, heal, `0x019b`, message and tail (432-454), as in capture 1.
* Level down never touched the worn Tsurugi (its minimum level is above 6): after `-90` to base 6 it stayed on; the `elvmax` check only looks at the maximum level. The later re-equip was refused with `0x0999` result 1 (line 1116).
* Aliases `@blevel`, `@job` work. `@iitem` (typo, line 592) is an unknown command (one message). `@job supernovice` is unknown because rAthena names use underscores.

**Capture 3** ([captures/attribute_sequence_3.txt](../captures/attribute_sequence_3.txt), write-up in [packets/status.md](../packets/status.md#capture-3-elvmax-unequip-hyper-novice-jlvl-observer)). Applies to 20260219.

* **`elvmax` unequip is the last step of `@blevel`.** `pc_baselevelchanged` runs after the tail (`SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `SP_BASEEXP`, `SP_NEXTBASEEXP`), so the unequip packets are the last of the command. A head-top item (Beginner Mark, max 99, comes off at base 100, lines 848-849) sends `0x099a` then `0x01d7` `LOOK_HEAD_TOP` 0. Shoes (Mana Boots, max 100, come off at 101, lines 880-881) send `0x01d7` `LOOK_SHOES` 0 then `0x099a`. The check is strictly `base_level > elvmax`: base 99 keeps the Mark, base 100 keeps the boots. Two items can come off in one command (loop over all slots); not captured. The unequip's own recalc sent nothing because neither item has a stat bonus. The observer gets only the `0x01d7`.
* **`@job` does not recheck `elvmax`.** Job changes at base 99 with both items worn kept them (lines 685-690).
* **Positive `n` clamps silently to the job's max**: `@blevel 2` from 98 on a Novice gives 99 with one "raised" message (465-495).
* **`@jlevel` area effect.** The positive case sends `0x019b` effect 1 to the area, so a nearby client gets it (line 931, first for the observer; the actor's copy is 932). Nothing else reaches the observer. The actor also gets any `0x0a24` achievement packets between the effect and the message.
* **Super Novice line.** `@blevel` runs no angel buffs (`pc_checkbaselevelup` does, `@blevel` does not call it) and the effect id stays 0 (not 7); no status icon appears.

### Config and permissions
* `conf/battle/gm.conf:44-47` `atcommand_levelup_events` (default `no`, `battle.cpp:8772`): when on, a positive change also fires the level-up NPC events. Otherwise no battle conf alters these commands. `multi_level_up*` and exp rates do not apply (direct set).
* Default atcommand levels: see the Permissions section above (`conf/atcommands.yml`).
* Persistence: nothing is saved by the command; `base_level`, `job_level`, exps, status/skill/trait points and stats ride the normal periodic/logout char save ([experience.md#persistence](./experience.md#persistence)).

## rust-ro (MIT, secondary) 

`rust-ro/server/src/server/request_handler/chat.rs` only treats a message as a command if it starts with `"<CharName> : @"` (fixed symbol, TODO configurable) and dispatches to `atcommand.rs`, which has `item`, `go`, `warp`, `blvl`, `jlvl`, `heal`, `speed`, `reload`, etc. (line 61 `"item" =>`, `handle_item` line 202). No group/permission check was observed. Useful only as a feature checklist.

## @resetstat / @streset / @reset (stat side)

Source: `conf/atcommands.yml:1220-1222` (command `resetstat`, alias `streset`), `atcommand.cpp:9648-9656`, `atcommand.cpp:7666-7673`, messages `conf/msg_conf/map_msg.conf` ids 206-208; applies at 20260219.

* `@resetstat` / `@streset`: `pc_resetstate(sd)` then `clif_displaymessage` msg id **207** (name arg). `@reset`: `pc_resetstate` + `pc_resetskill(sd,1)`, msg id **208**. `@resetskill`/`@skreset`: skills only, msg 206. No arguments, no failure case.
* `pc_resetstate` effects and push order (stats 1/0, status and trait pools from the tables, +`trait_points_job_change` for trait jobs, +`transcendent_status_points` for upper/4th): [trait_stats.md#3-trait-points](./trait_stats.md#3-trait-points). The message is sent last, after the reset's own recalc diff.
