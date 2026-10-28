# Deferred work

Items consciously left out of finished work, with enough context to pick them up later.
Remove an item when it lands. Newest feature first.

## stats (branch `main`, plan `specs/stats/plan.md`)

Spec: `specs/stats/spec.md` §5. Job and refine data, every derived stat from job, level, allocated stats and
gear (ATK/MATK/DEF/MDEF only), the tick's recalc pass with the follow-up queue, stat-up, the real `0x0141`,
`@blvl`/`@jlvl`/`@job` with the gear re-checks, and saving job, levels and stats are in.

### Still to verify by hand
- **Manual client check:** the user's metaling recording
  (`reference/catalog/captures/metaling_attribute_sequence.txt`, 2026-10-28) matched captures 1-3 in every
  flow, apart from documented deviations. Still worth checking by eye in the client: an equip of a Knife +10
  or Guard (one net diff) and the status window after `@jlvl` (job bonus shown as `+`).
- Char select (`0x0b72`, `crates/ro-packets/src/outgoing/char.rs`) still sends placeholder HP/SP
  (100/100/50/50) and 0 skill points. The real values are saved now; the char server doesn't load or send
  them yet.
- Not captured: a stat above 255 (unreachable with the shipped caps of 130 or less; `FollowUp::CoupleStat`
  covers it from the source), two `elvmax` items coming off in one command, and `@job`'s ammo refusal
  `0x0291` before the forced unequip (source reading only).
- A Knight wearing a katar showed amotion 1250 in capture 1, where the catalog predicts 2040. rAthena
  recalcs before its forced unequip; ours runs after, so we never compute ASPD for an unusable weapon.

### Modifier sources and mechanics
- Item, card and combo bonuses, and gear hard MDEF (`crates/ro-engine/DESIGN.md` §9.8). Passive skills
  (§9.9). Status effects (§10), including restoring saved ones at login (capture 2's late `SP_ASPD` is a
  Berserk Potion buff). Damage (§11). Mob `Status` from `MobDb`.
- `die_counter` (the Super Novice +10 assumes no deaths), Taekwon ranker HP ×3, enchant grade, overrefine
  damage.
- Exp tables (NEXTBASEEXP/NEXTJOBEXP stay placeholders), exp gain and natural level-up.
- Natural regen, overweight statuses, trait stats and AP. `SP_TRAITPOINT` is sent as 0; rAthena sends the
  running trait total.

### GM commands
- `@job`: class-change level vars; the skill tree and skill deletes (`0x0441`; the `0x010f` re-send is
  done); option stripping (cart, falcon, peco, mado, homunculus); transcendent (±52) and trait (+7) point
  grants and their stat resets; the party class update; the upper/baby flag argument.
- `@jlvl` negative: `pc_resetskill` (no skills yet).
- Level effects not sent: achievement updates (`0x0a24`), the `0x0446` quest marker refresh, and the
  observer's party HP bar `0x080e` (capture 3).
- `LOOK_CLOTHES_COLOR` is always 0 in `@job`'s looks: no cloth colour is stored yet, so a dyed character
  would lose its colour until relog.

### Engine (`crates/ro-engine/src/{stats,status,wear}.rs`)
- An `elvmax` unequip runs as a follow-up, so its own stat diff goes out next tick. The captured items had
  no stats, so nothing was visible; an `elvmax` item with stats would show its diff one tick late.
- `status_diff` allocates two `Vec`s per recalc (dirty players only; idle ticks allocate nothing).
- `wear.rs`: `DISPLACED` and `CASCADED_AMMO` are now identical; merge them or give them a reason to differ.

## equipment (branch `main`, plan `specs/equipment/plan.md`)

Spec: `specs/equipment/spec.md` §5. Equip, take-off, unequip-all, dual wield (Assassin/Kagerou class), ammo
with its icon and cascades, looks (live, entry packets, char select), the gear-only status push, and a
dropped worn stack coming off first are in.

### Still to verify by hand
- **Manual client check** (plan Finish): `@item` a knife, a guard, a hat; equip, swap, take off, unequip
  all; an observer sees the looks; relog keeps the gear; char select; an Archer (`UPDATE characters SET
  job_class = 3`) with a bow and arrows.
- A shield replacing a bow takes the arrows off (our ruling; not captured).

### Needs later stats work
- Grade bonuses to ATK/DEF/MATK (enchant grade; refine landed with the stats feature).

### Item features
- Item scripts and bonuses (item MDEF is script-only in renewal data, so `hard_mdef` stays 0); cards and
  combos; `EquipScript` / `UnEquipScript`. Bravery Bag's skill-grant packets come from its script.
- Bind-on-equip (`bound = 4`, `clif_notify_bindOnEquip`).
- Equip switch (`0x0a9b` contents, `equip_switch` column use).
- Pet armor.
- Identification: mob-dropped equipment is unidentified, so it fails with result 2 until identify exists.

### Gates
- Strip / `cant.equip` / `SC_PYROCLASTIC` / `SC_KYOUGAKU` blocks; map `NoEquip`; `PCBLOCK_EQUIP` and
  NPC-script gates with `MSI_CAN_NOT_EQUIP_ITEM` (372); `canequip_tick`.
- `AS_LEFT` dual wield for non-Assassins; the `AS_VENOMKNIFE` dagger-ammo rule.
- Soul Linker Super Novice shortcuts; the `PC_PERM_USE_ALL_EQUIPMENT` GM bypass.

### Packets
- `skillrange_from_weapon` skill-list re-send on a range change.
- The ammo EFST end (`0x0196`): rAthena never sends it. Revisit if the client keeps the icon after
  unequip.
- Entry `body` for NPCs with a player sprite: rAthena sends their class there (catalog `map_units.md`); we
  send 0. Only matters once such NPCs show a job body.
- Bullets and cannonballs are untested against a capture (only arrows are captured); the `0x099a` failure
  forms are from source reading.

## map-entry-settings (branch `main`, plan `specs/map-entry-settings/plan.md`)

Spec §1–§5. The settings group is sent with defaults, `0x0447` gets its `0x0b1b`, (Fix 1) the login also
sends NV_BASIC level 0 and the exp/status block, and (Fix 2) the login gets the pre-`0x007d` burst with the
login-time `0x0091`, and every entry starts with `0x01d7` and sends `0x099b`.
- **Manual client check passed** (2026-10-24, `reference/catalog/captures/metaling-20261024-4.txt`): two `0x007d`
  after the login `0x0091`, then `0x0447` → `0x0b1b`; the chat box takes input; mobs stay visible after the reload
  (the client asks their names later).
- **A chat-related error is still shown** after login. The user's guess is chat settings. The capture has no
  unanswered client packet that hints at it (only `0x08c9` cash shop list and `0x0b1c` pings go unanswered, and
  rAthena answers neither in its captures). Every packet rAthena sends that metaling never sends is either a
  proxy misframe or belongs to a feature metaling doesn't have yet (party `0x080e`/`0x0101`, mob HP `0x0977`,
  exp gain `0x0acc`, `0x0088`, `0x00b2`). Next steps: record the exact error text, check whether the same client
  shows it on rAthena, then ask `metaling-explorer` which packet carries it. Candidates: the `0x0a23` achievement
  list (rAthena sends it after login auto-completions), `0x0adf` name pushes, or a chat/UI-settings packet the
  proxy hides inside a misframe.
- Optional: bisect which Fix 2 packets are needed. The login `0x0091` is the one that unblocked input.
- Logging in on a same-map warp cell: the first `0x007d` warps the player, the client's second `0x007d` (the
  login reload) then runs the destination's actor init, and the warp's own `0x007d` only re-sends the inventory.
  Input should work, but the client may miss the destination's unit entries. rAthena has the same order.
- Fix 2 simplifications (`crates/ro-engine/src/settings.rs`, `status.rs`):
  - `0x01d7` at entry goes to the player only; rAthena also sends it to observers in view, just before their
    `0x09fe`.
  - The MOTD is one fixed line (`MOTD`), not configurable.
  - `0x0a23` is never sent (rAthena skips it with no achievements; it sends it after login auto-completions).
    `0x0a24` carries zeros and next-level exp 18.
  - The pre-`0x007d` status push sends every field once in captured order, not a diff. rAthena sends
    DEF2/DEF1 twice.
  - `0x099b` is the plain-map form (`property 0, flags 0x0600`) on every map until mapflags exist.
  - rAthena pushes `0x0adf` name replies after mob entries unasked; metaling still answers `0x0368`.
- Char server: metaling's `0x0b72` page is 879 bytes and rAthena's 179 (handoff item 3). Not compared yet,
  nor `0x0ac4`, `0x082d` and `0x006b`.
- Save and load hotkeys (`CZ_SHORTCUT_KEY_CHANGE*`, row shift) and the config toggles (`CZ_CONFIG`,
  `CZ_PARTY_CONFIG`, equipment window).
- A real skill list (NV_BASIC's real level and job skills) with the skill tree. Today every character gets
  NV_BASIC level 0, upgradable.
- Status block placeholders (`crates/ro-engine/src/status.rs`):
  - NEXTBASEEXP/NEXTJOBEXP are the max-level constants until exp tables are imported.
  - Trait stats, their costs and AP are 0.
- Reputation points (always 0) and the reputation types list (fixed 1–4 in `ro-packets`).

## items (branch `main`, plan `specs/items/plan.md`)

Spec: `specs/items/spec.md` §5 lists these too. Inventory, floor items, pickup, player drop, the mob-drop
API and `@item` are in.

### Still to verify by hand
- **Manual client check** (plan Finish): account `test` (group 99): `@item 501 5`, `@item Knife`,
  `@item "Red Potion"`; relog (persisted); warp (kept); drop (the observer sees it); pick up; walk away and
  back (`0x009d` / `0x00a1`); 60 s expiry; normal chat; a non-GM `@item` shows as chat.
  Partly done on 2026-10-24 (`metaling-20261024-4.txt`): `@item knife` / `@item blade` as GM, drops and
  pickups seen by a second client, a non-GM `@item` shown as chat. Still to check: `@item 501 5` and a quoted
  name, relog (persisted), warp (kept), walking away and back, 60 s expiry.
- **Recapture on rAthena** with the fixed proxy table. Recorded 2026-10-24 as
  `reference/catalog/captures/packet_rathena_item_2026-10-24.txt` (has a drop and its `0x00af` reply, line 567);
  not yet compared against the catalog. Still check: a drop (the `0x00af` reply, PF-21), walking back to
  an old drop (`0x009d`, PF-03), an overweight or full pickup and an early pickup (the `0x0b41` failure
  form, PF-07).

### Mob drops
- Call `MapInstance::drop_mob_items` at mob death (combat), with owners from the damage log, in the death
  tick after the exp packets (PF-15).
- The renewal drop level penalty and player drop-rate bonuses (`roll_mob_drops` has the slot for them).
- MvpDrops and MVP rewards (imported, unused).

### Looter mobs
- `MD_LOOTER` (Ai 02, e.g. the Poring) picks up floor items; its `lootlist` drops at death with
  `loot = true` (first loot item goes N, no effect). `tools/mob-import` will need the looter bit.

### Item features
- Item use `0x0439` / `0x01c8`, `0x07fa` deletes, item scripts, cooldowns.
- Rental items: `expire_time` timers and the rental-expired packets.
- Overweight statuses `SC_WEIGHT50` / `SC_WEIGHT90`, and `0x0ade` at entry.
- Inventory expansion: `inventory_slots` and a non-zero `0x0b18` (the persistence load refuses more than
  100 rows today).
- Max weight: the per-job `MaxWeight` (`job_stats.yml`) and bonuses (Enlarge Weight Limit, mounts).
- Autoloot and party loot share (`party_share_loot`, party item-share tiers in the loot check).

### Gates and commands
- The `NoDrop` GM override and group levels, the `nodrop` mapflag, `pc_cant_act` states (NPC dialog,
  trade, vending, storage, opt1), and `sc.cant.pickup` / `sc.cant.drop`.
- `@item2`, `@itembound`, pet eggs (`pet_create_egg`; `@item` refuses them with our own message), the rest
  of the command list, and a real `groups.yml`-style permission model (today: group 99 = everything).
- `#` char commands are plain chat.

### Persistence (from the M5 review)
- Position and inventory are saved in two transactions through one ordered writer. A crash between the two
  writes of a logout keeps the new position with the old inventory (items gained since the last save are
  lost, never duplicated). A combined `save_character_state` would close that.
- Slot order on relog relies on identity values following `INSERT … ORDER BY`. An explicit `slot` column
  would not.
- A multi-process hand-off needs DB-level ordering (e.g. a version column checked in `save_inventory`); the
  in-memory writer orders one process only.
- Autosave saves the inventory only; rAthena's autosave also saves position and stats.

## mob-spawning (branch `claude`, commits from `563e8a7`)

Spec: `specs/mob-spawning/spec.md` (§4 lists these too). Mobs are visible, named, inert units.

### Still to verify by hand
- **Real-client check** (plan Finish, optional): walk `prt_fild08` against metaling and rAthena through
  `tools/proxy`. Compare the `0x09ff` fields and the `0x0adf` reply. The wire values come from source only
  (catalog `mechanics/mobs.md` §1.5).

### mob_db stats
- Real `Hp`, `Level`, `WalkSpeed` and `is_boss` (1 for `Class: Boss`, 2 for `Mvp`). Today every mob has
  1 HP, level 1 (or the line's level) and speed 150, and `is_boss` is always 0. **The data is imported now**
  (`crates/ro-db/data/mob/mob_db.yml`, `ro_db::MobDb` with `boss_type()`, shared with every
  `MapInstance` via `set_registries`); only the wiring into `spawn_mobs` / `MobInfo` is left.
- `GroupId` / `Title` in `0x0adf`. Today both are empty.

### Death and respawn (need combat)
- **Respawn delays:** `delay2` counts only for `boss_monster` lines (`mob_spawn_variance: 1`); a given
  `delay1 < 1000` becomes 5000; rate scaling; a 1000 ms floor. `SpawnGroup.line` keeps the raw values.
- **On (re)spawn while players are present:** `0x09fe` to `AREA_WOS`, then the name to `AREA`.
- **Event labels** are stored in the YAML (`Event`) but nothing runs them. 139 lines have an event field,
  and 44 of those carry a real label such as `"ant_d02_i_boss::OnMobDead"` or
  `"illusion_mob#moonlight::OnKill"`. When the mob dies, rAthena runs that NPC label with the killer
  attached (catalog `mechanics/mobs.md` §3 step 9). This needs mob death (combat) and the scripting VM.
  Quest and instance scripts that count kills this way won't progress until then.

### Spawning gaps
- The 1000 ms retry after a failed placement (ENG-MOB-03): the mob is simply not created.
- `dynamic_mobs` removal 300 s after the last player leaves, and `MAX_MOB_LIST_PER_MAP` (128 lines). Map
  worlds start on first entry, so spawning at world start already matches rAthena's first-entry spawn.
- The spawn line's `size`/`ai`, and the `EF_GIANTBODY2` / `EF_BABYBODY2` effects (to the area on spawn,
  to that player only on view-enter). No current data sets them; the importer warns and drops them.
- The config knobs `mob_count_rate`, `force_random_spawn`, `no_spawn_on_player` and `override_mob_names`
  (all at rAthena defaults).
- Once-spawns: the script `monster` command and `@monster`.
- Mob gids are per map instance (rAthena: one global pool shared with NPCs).

### Placement fidelity (`crates/ro-engine/src/mob.rs`, from the final review)
- **Per-axis map-wide search.** When one search radius is negative (a line like `map,50,0` gives span
  `(1, 0)`), `search` goes map-wide on both axes; rAthena decides per axis. The shipped data has no such
  line.
- **Boss 1x1 shortcut on a zero coordinate.** The catalog says "fixed 1x1 spawn on a nonzero cell"; we
  only check `span == (1, 1)`, so `boss_monster` at `map,0,50,1,1` takes the shortcut. Unclear whether
  rAthena needs both coordinates nonzero (ask `metaling-explorer`). No data hits it.

### Performance and robustness
- `MapInstance::gid()` checks three sparse sets (players, NPCs, mobs) on every step. Fine while mobs don't
  move; consider a dense gid column when mob movement or AI lands.
- `get_or_spawn_world` loads `.gat`, NPC and mob files and spawns synchronously on a tokio worker (already
  true for NPCs). Consider `spawn_blocking`.

### Movement and AI
- No random walk, chase or AI timers. Catalog `mechanics/mobs.md` §1.7 has the rAthena rules: the
  4000 ms + `rnd()%1000` walk timer, hard AI (100 ms) only within 18 cells of a player, lazy AI (1000 ms)
  otherwise, and `mob_randomwalk`'s ±7 window.

## exit-game (branch `claude`, commit `d666b53`)

Spec: `specs/exit-game/spec.md` (§3 lists these too).

### Still to verify by hand
- **Real-client check** (plan M5). "Exit to Windows" through `tools/proxy` should close the client, and
  the observer should see `0x0080` type 2. The character should then log back in at the cell where it quit.

### Robustness (`bins/map-server/src/{service,session}.rs`)
- **Exit or restart racing a cross-map handoff.** If map A hands the session off just as `0x018a`/`0x00b2`
  arrives, the session task can still forward it to A, which drops it. A now logs this at `warn`.
  - Effect: no ack. The client stays in game, loads the destination map and plays on. Pressing Exit
    again works. Server state stays consistent: the claim moves to B as in any warp.
  - The same window applies to every intent. Fix it in the handoff, e.g. A keeps handed-off session ids
    for one tick and forwards their intents.
  - Test: warp and quit in the same tick, then assert `0x018b` arrives.

### Needs combat
- **Refusing the exit (`0x018b` result 1, "wait 10 seconds").** rAthena refuses while the character is
  hidden or cloaked, or within `prevent_logout` (10 s) of attacking, casting or taking damage. The same
  check refuses `CZ_RESTART`.
- **Delayed removal.** A client that closes inside the `prevent_logout` window stays in the world for 10 s.

## warp-map-change (branch `claude`, commits from `2a7904f`)

Spec: `specs/warp-map-change/spec.md` (§3 lists the scope cuts).

### Out of scope
- **Script portals stay inert.** About 970 positioned `script` NPCs with warp sprites (45/139) do `warp` from
  `OnTouch`; only plain `warp`/`warp2` entries carry a `WarpTrigger`. They need scripting. Warp skills
  (Teleport, Warp Portal) and `@warp`/`@go` too.
- **Entry content:** rAthena's login-time `0x0091` ([PF-20](PACKET_FINDINGS.md#pf-20)), `0x01d7` to observers
  at entry ([PF-01](PACKET_FINDINGS.md#pf-01)), and the inventory re-send at entry.
- **Hiding.** There is no hiding state, so the "hidden players don't warp unless `warp2`" skip is missing;
  `WarpTrigger.trigger_while_hidden` is stored but unused.
- `pc_job_can_entermap` (job/group restrictions on the destination).
- **Multi-map-server handoff** (char-server reconnect) and instance (`1@...`) destinations: instance
  warps are inert at load.

### Robustness
- **Blocking map load on the session task.** `get_or_spawn_world` reads the `.gat` and NPC files on the
  calling task; a warp to a map not yet running now does this on the session task.
- **Re-login on a warp cell after a disconnect in transit** (spec §1 deviation): `rewarp_count` resets,
  and a warp disabled later leaves the player standing on it.
- With `MAP_DATA_DIR` set, a map whose own `.gat` is missing (fallback grid) treats its same-map warps as
  inert too.
- **Packets while loading.** rAthena drops everything but `0x007d` while the player is off the map. We
  still answer `0x0360` (as the spec allows) and act on Restart `0x00b2` on the destination before
  `0x007d`.

## npc-spawning (branch `claude`, commits `1111051..a16d629`)

Spec: `specs/npc-spawning/spec.md` (§4 lists these too).

### Data not converted (tools/npc-import)
- Script bodies (and where the scripting phase reads them from: `.txt` via `Source:`, or extracted files).
- Floating (`-`) NPCs and `function`s, mapflags, shop catalogs.
- Barter NPCs defined in `npc/**/barters.yml`.
- rAthena drops shops with zero valid items; we keep every shop (no item_db yet).

### Fidelity
- NPC names go on the wire as UTF-8; the client expects CP949 (6 headers in `quests_17_2.txt` /
  `enchan_sage_legacy_17_2.txt` use Greek letters).
- PC-job-sprite NPCs: rAthena also sets `body = class` and clamps hair; guild-flag NPCs send emblem data.
- `Dir` values above 7 (8, 15, 55 in data) are sent masked to 4 bits; the client's handling is unverified.
- NPC ids are per map instance (rAthena: one global pool shared with mobs/pets).
- Name requests answer only NPCs in the spatial grid (rAthena also answers disabled NPCs).
- The engine treats every sprite except `-1`/`32767` as visible. rAthena hides classes outside its view
  ranges (`mechanics/npcs.md`); the current data has none, but new data could.
- Real-client proxy capture of NPC entry (`0x09ff` object_type 6) and `0x0adf`: everything is source-derived.

## engine-foundation (branch `claude`, commits `ed7c0d9..af3445d`)

Spec: `specs/engine-foundation/spec.md` (§6 lists the scope cuts below as well).

### Still to verify by hand

- **Real-client proxy check** (plan "Final checks"): run two real clients through `tools/proxy` against
  metaling and against rAthena (walk in and out of view, sit, turn, name request) and diff the captures in
  `tools/proxy/packet_captures/`. Not done: needs the game client.

### Robustness (map-server, `bins/map-server/src/{service,session}.rs`)

- **Pre-auth handshake timeout.** A truncated `CZ_ENTER` with no EOF keeps the socket open forever
  (slowloris). BIN-01 was reworded to "truncated `CZ_ENTER` then EOF".
- **Duplicate-login policy and idle timeout.** If a half-open socket still holds a character, a reconnect
  is rejected (spec BIN-01) until the old TCP connection dies. rAthena kicks the old session or sends a
  duplicate-login notice instead.
- **World spawn cap on entry.** `resolve_entry` starts a world for any valid stored `last_map` without the
  `MAX_HOSTED_WORLDS` check that `accept_transfer` does.
- **Map-side auth.** `login_id1` is not checked against a char-server ticket (needs a handoff that doesn't
  exist yet). Until then anyone can send `CZ_ENTER` for any visible account/char id pair.
- **Slow-client eviction scenario and tick-budget assertion** (old BIN-SRV-05). The `try_send` eviction
  exists but has no dedicated test.

### Engine fidelity (`crates/ro-engine`)

- `OFFICIAL_WALKPATH` straight-line rule for 15–17 step paths.
- rAthena's 32x32 A* node-table collision quirk and equal-cost tie-breaking parity.
- Sub-cell offsets other than (8, 8) in `0x09fd` (rAthena uses them when a walk is cut short mid-cell).
- Walk speed clamp: speed 0 from the DB makes every step complete in one tick (rAthena clamps speed).
- Weight / max weight are placeholders (`0` / `20300`) until inventory and stats exist.
- Mobs, status, combat, and rayon parallelism are future phases (`DESIGN.md`).
- `.rsw` water-level promotion of type-0 cells (`ro-db`).

### Performance (hot path; CLAUDE.md map-server rule)

- A* output is copied twice (scratch → `PathCells` → `WalkingPath`); build `WalkingPath` from the slice.
- The actor encodes an event before checking that any recipient is still joined.
- Spatial blocks are `SmallVec<[Entity; 8]>` (~295 KB for a 512x512 map); an intrusive list (`u32` head
  plus a `next` column) would shrink this if many maps run.
