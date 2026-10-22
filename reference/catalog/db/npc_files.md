# rAthena NPC Source Files (`npc/**/*.txt`, `*.conf`)

Grammar notes for an offline converter that extracts only the headers of positioned NPCs.
Written from rAthena commit `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39` (RENEWAL build). Line numbers refer to that commit.
All of this is server-side data and parsing logic, independent of `PACKETVER`; it applies to `20260219` as is.
Sources: `$RATHENA_PATH/src/map/npc.cpp` (NPC parse), `src/map/map.cpp` (manifest), `src/map/script.cpp` (`skip_space`), `src/common/strlib.cpp` (`sv_parse`), `src/map/npc.hpp` (sprite enum).
Counts in the last section were produced by a throwaway scanner that reimplements these rules (not by rAthena).

## 1. Manifest (which files load)

- RENEWAL entry point is exactly `npc/re/scripts_main.conf` (`map.cpp:4258-4267`, `map_reloadnpc`, called once at startup from `map.cpp:5363`). Pre-renewal uses `npc/pre-re/scripts_main.conf` under `#else`; nothing in the renewal manifest references `npc/pre-re/`, so it is excluded by construction. There is no `npc/scripts_main.conf`.
- `conf/map_athena.conf` does not list NPC files (only `import: conf/maps_athena.conf` and `conf/import/map_conf.txt`, lines 126-128). Its `npc:` / `delnpc:` keys are still honoured by the map config reader (`map.cpp:4170-4173`) if a user adds them (`conf/import/map_conf.txt`), and those are applied to the same list. Default repo has none.
- `scripts_main.conf` lists 2 direct `npc:` files (`npc/other/Global_Functions.txt`, `npc/other/CashShop_Functions.txt`), then `import:` of common `npc/scripts_{athena,guild,jobs,mapflags,monsters,warps}.conf`, then the same six under `npc/re/`, then `npc/scripts_custom.conf` (all entries commented out by default). `scripts_test.conf` import is commented out.
- Directive line parser (`map.cpp:4216-4256`, `map_reloadnpc_sub`):
  - A line starting with `//` is skipped. Otherwise everything from the first `//` onward is cut, so a path containing `//` is truncated.
  - Format is `key: value`: key is everything up to the first `:` (not trimmed, so leading spaces make the key unknown), value skips spaces after the colon and runs to tab/CR/LF; trailing spaces are trimmed. Lines without a colon are ignored.
  - Keys (case-insensitive): `npc` (add file), `delnpc` (remove file), `import` (recurse into another conf now). Any other key prints "Unknown setting" and is ignored.
  - Special values: `npc: clear` empties the list, `delnpc: all` empties the list (`npc.cpp:3606-3641`).
- Paths are used verbatim as given and are relative to the server working directory (the rAthena repo root). No normalisation: `delnpc` only matches a byte-identical string.
- Order semantics: `npc:` appends to an ordered list unless the identical string is already in it (silently ignored, so the same file cannot load twice via `npc:`) and the path must be a regular file or it is rejected with an error. `delnpc:` removes the entry if present at that moment; a later `npc:` re-adds at the end. `import:` has no dedupe (a conf imported twice re-runs its lines, but `npc:` dedupe still protects the file list). Files are then parsed in list order (`npc_loadsrcfiles`, `npc.cpp:3646`), which matters for `duplicate(...)` (source must exist already) and unique-name collisions.
- Separate from the text manifest: `npc/barters.yml` (`npc.cpp:395`) creates positioned barter NPCs from YAML (`Name`, `Map`, `X`, `Y`, `Direction`, `Sprite` name, defaults not-on-a-map / 0 / North / FakeNpc). There is no `trader` or `barter` keyword in any `.txt` NPC file; `grep` over `npc/` finds none.

## 2. Lexing rules (`npc_parsesrcfile`, `npc.cpp:5646-5819`)

- Whole file is read as bytes into memory. A UTF-8 BOM (EF BB BF) at offset 0 aborts the file. Encoding is not interpreted (treat as Latin-1/bytes; names are ASCII in practice).
- Loop: `p = skip_space(p)` before every record, then the record is the rest of the current line.
- `skip_space` (`script.cpp:866`): skips whitespace (including newlines, and leading tabs/spaces before a header), `// ...` to end of line, and `/* ... */` which may span lines and may sit between records. An unterminated `/*` consumes the rest of the file silently (warning only). Comments are recognised only at record starts and inside script bodies, NOT inside a header line: a trailing `// text` on a header line stays in the last field (harmless because field 4 parsers read only the leading tokens; see sprite rules).
- Header split (`sv_parse`, delimiter `\t` only, no quoting, no escapes): line is terminated by LF or CRLF (`\r\n`; a lone `\r` is not a terminator and stays in the data). Fields 1-3 are single tab-delimited fields; field 4 (`w4`) is everything after the third tab to end of line, including any further tabs. Spaces are not delimiters. Empty fields count (a double tab produces an empty field, shifting later columns; this makes `w4` start with a tab, see sprite surprise in section 5).
- Fewer than 3 fields: "Unknown syntax" error and the whole REST OF THE FILE is abandoned. Field count must be > 3 for warp, shops, script, duplicate, monster; mapflag needs >= 3.
- Dispatch on `w2` (case-insensitive prefix tests, order matters): `warp*` (prefix `warp` or `warp2`) -> warp; exact `shop|cashshop|itemshop|pointshop|marketshop` -> shop; `script*` (prefix, so `script(HIDDEN)`) -> script or, if `w1` is `function`, user function; `duplicate(` -> duplicate; exact `monster|boss_monster` -> mob spawn; exact `mapflag` -> mapflag; anything else: error and skip the line only.
- `w1` pre-check (not for `-` and `function`): `map,x,y` parsed with sscanf (`%15[^,],%6hd,%6hd`); unknown map name (not in the map index), map not loaded on this server, or x/y outside the map grid skip the entry (for scripts the body is skipped with `npc_skip_script`). Maps are matched against the server's map list, so a converter must decide itself which maps to keep.
- Script body end (`npc_skip_script`, `npc.cpp:4311-4372`): start at the first `{` found after the header's `,{`. Then repeatedly `skip_space` (so `//` and `/* */` comments inside bodies, even containing braces or quotes, are ignored), and: `}` decrements depth, `{` increments depth, `"` starts a string. Strings: `\` skips the next char (unless the previous byte is >= 0x7f, treating it as a multibyte trail); a raw newline or EOF inside a string is a fatal error for the rest of the file. Single quotes are NOT special (a `'` is just another char; a lone `{` or `}` inside `'...'` would miscount). Nothing else is tracked (labels, `case`, etc.). Body ends at the matching `}`; parsing resumes right after it, so another record may follow on the same line, and text after the closing brace (e.g. `// comment`) is consumed by the next `skip_space`.
- Errors that stop the whole file: fewer than 3 fields; script header with bad `w1` placement (not exactly 4 fields), missing `,{`; unterminated body or string.

## 3. Header formats

Columns are `w1<TAB>w2<TAB>w3<TAB>w4`. `w1` is the location.

Location `w1`: `<map>,<x>,<y>,<dir>` or `-` for floating (only scripts, shops, duplicates of those, and `function`). Map name is read up to 15 chars (`1@mcd`, `rgsr_in`, instance names with `@` exist). x,y,dir are `%hd` (whitespace tolerated before numbers, e.g. `rgsr_in,135,186, 7`; extra trailing fields are ignored, e.g. `1@mcd,144,212,5,0`). Floating means map id -1, x=y=dir=0, class forced to FAKENPC (sprite ignored).

| Kind | `w2` | `w1` | `w3` | `w4` |
| :--- | :--- | :--- | :--- | :--- |
| warp | `warp` or `warp2`, optional state suffix `(CLOAKED)`, `(HIDDEN)`, `(DISABLED)` | `map,x,y` required; dir is NOT read (field usually `,0`; may be missing in principle) | name | `xs,ys,destmap,dx,dy` (all 5 required) |
| script, positioned | `script`, optional `(CLOAKED|HIDDEN|DISABLED)` | `map,x,y,dir` (all 4 required) | name | `<sprite>,{` body ... `}` |
| script, with OnTouch area | same | same | same | `<sprite>,<xs>,<ys>,{` body `}` (xs,ys signed 16-bit, `-1` = none) |
| script, floating | `script` | `-` | name | `<sprite>,{` ... (sprite value is ignored; commonly `-1`, `FAKE_NPC`, `HIDDEN_NPC`, `722`) |
| user function | `script` | literal `function` (case-insensitive) | function name | `{` ... `}` (no sprite; needs `<TAB>{` on the same line) |
| shop / cashshop / itemshop / pointshop / marketshop | exact keyword | `map,x,y,dir` (4 required) or `-` | name | `<sprite>,<items>` where items are `id:price` (market: `id:price:qty`); itemshop adds `,<costitemid>[:discount]` first, pointshop `,<variable>[:discount]` first. Shop with zero valid items is DISCARDED (not registered). `price` `-1` = item default |
| duplicate | `duplicate(<src>)` | `map,x,y,dir` or `-` (floating only allowed when the source is a shop-type or script) | name | warp source: `xs,ys` (required); script source: `<sprite>[,<xs>,<ys>]`; shop source: `<sprite>`; sprite always the first token |
| monster / boss_monster | exact | `map[,x,y[,xs,ys]]` | `name[,level]` | `mobid,amount[,delay1[,delay2[,event[,size[,ai]]]]]` |
| mapflag | `mapflag` | `map` (coordinates optional) | flag name | `on|off` or flag argument |

- `warp2` differs only by `trigger_on_hidden = true` (can be triggered while hidden); `warp` has it false (`npc.cpp:3935-3940`). Both may carry a state suffix. Duplicates of a warp copy `trigger_on_hidden`.
- State suffix: `script(CLOAKED)`, `script(HIDDEN)`, `script(DISABLED)`, `warp(DISABLED)`, `warp2(DISABLED)` set the initial NPC view state (cloak/hide/disable). Unknown state: warning, defaults to visible. Duplicates inherit the source's state; duplicate headers do not take their own suffix.
- `duplicate(<src>)`: the sprite token is NOT optional for positioned script/shop duplicates (it is read from `w4`; floating ones ignore it but data still writes `-1` or a number). For warp duplicates `w4` is only `xs,ys`. Source must already be registered: lookup is `npc_name2id(src)` over the unique name (exname) table, global across all files loaded so far, case-sensitive; missing source logs an error and skips the line. Source name length limit 127, `src` taken between `duplicate(` and the final `)`. A duplicate of a duplicate works (resolves to the root `src_id`).
- `trader`/`barter` are not text keywords. The non-NPC headers a converter must recognise-and-skip are `function`, `monster`, `boss_monster` and `mapflag`. `setcell`, `unloadnpc` etc. are script commands inside bodies, not header keywords.
- Dir: no validation. rAthena keeps the raw value as `uint8` (`(uint8)dir`). Valid facing is 0-7 (0=N, then counter-clockwise: 1=NW, 2=W, 3=SW, 4=S, 5=SE, 6=E, 7=NE). The default manifest contains out-of-range values on positioned entries: `8` (87 entries), `15` (1), `55` (2). Warps carry dir 0 in practice.
- `0,0` coordinates: only meaningful for floating (`-`) entries and for mapflags without coordinates; rAthena treats `-` as map -1 and x=y=0. A positioned entry at x=0,y=0 is valid if inside the map.

## 4. Name rules (`npc_parsename`, `npc.cpp:3668-3718`)

- `w3` forms: `Display#Hidden`, `Display::Unique`, or plain. Name is split at the first `::` into display (`nd->name`) and unique (`nd->exname`); without `::`, both `name` and `exname` are the full string (including any `#...` suffix).
- `#` suffix handling is client-side: the server stores the whole string, the client shows only the part before `#` (convention `Visible#hidden_id`). A name that begins with `#` (e.g. warps `#to_lhz`) displays as empty.
- Max length `NPC_NAME_LENGTH = 50` (`common/mmo.hpp:157`; buffer 51 incl. NUL; `NAME_LENGTH` is 24). Longer display or unique names are truncated with a warning. `MAP_NAME_LENGTH` is 12 (`_EXT` 16). No default-manifest name exceeds 50.
- Empty unique name or one still containing `::` becomes `0_<mapid>_<x>_<y>`.
- Uniqueness: `exname` must be unique across ALL NPCs (including floating ones and duplicates); the lookup table is case-sensitive. On collision the new NPC gets `exname = <n>_<mapid>_<x>_<y>` (n = first free counter) with a warning; the earlier NPC keeps its name; display name is unchanged. The default renewal manifest has zero collisions (2 pairs of names differ only by case, which are distinct keys).
- Display names may contain spaces (7166 of the entries) and `#` (18669).

## 5. Sprite field

- First token of `w4` for script/shop/duplicate (warps have none). The token ends at the first whitespace, `/`, or `,` (`npc_parseview`, `npc.cpp:3744-3780`), so trailing `// comments` and `,xs,ys` are ignored.
- Resolution order: (1) `strtol(token, base 0)` over the whole token, so decimal, `0x` hex, and leading-0 octal (`01` = 1, `010` = 8) all parse; (2) script constant name (case-insensitive `strcasecmp` lookup, `script.cpp:690-699`); (3) mob AegisName from `db/re/mob_db.yml` (`mobdb_search_aegisname`, case-insensitive); (4) otherwise warning and `JT_INVISIBLE` (32767). Empty token (e.g. `w4` begins with a tab) parses as number 0 (sprite 0 = NOVICE job) - this actually occurs once: `Rebellion Crew#171_rebelin` in `npc/re/quests/quests_17_1.txt` (w4 is `<TAB>CLEAR_NPC,`).
- Constants: enum `e_job_types` in `src/map/npc.hpp:247-1584`; each member `JT_<NAME>` is exported to scripts with the `JT_` prefix stripped (`script_constants.hpp:7483-7489`, `export_constant_npc`; 1313 `JT_` exports). To extract a name->id table mechanically: parse that enum, strip `//` and `/* */` comments, split on commas, an explicit `= N` resets the counter, otherwise previous+1; strip the 3-char `JT_` prefix; compare case-insensitively. Enum has 1320 members (including the 6 `NPC_RANGE*` markers), range -1..32767. Mob AegisNames map to the mob id (1001+). `db/const.yml` is not needed for sprite names in the default manifest.
- Values: `WARPNPC` = 45, `HIDDEN_NPC` = 111, `HIDDEN_WARP_NPC` = 139, `GUILD_FLAG` = 722, `INVISIBLE` (`JT_INVISIBLE`) = 32767, internal `JT_FAKENPC` = -1 (not an exported script constant; `FAKE_NPC` appears in data only on floating entries where it is ignored, and `FAKE_NPC` is also the name of an internal fake NPC, `npc.cpp:6373`). Valid NPC view id ranges (`npcdb_checkid`, exclusive bounds): 45-125 (`NPC_RANGE1`), 139, 401-999 (`NPC_RANGE2`), 10001-19999 (`NPC_RANGE3`), 32767. Enum counter markers: `NPC_RANGE1_START`=44, `NPC_RANGE1_END`=126, `NPC_RANGE2_START`=400, `NPC_RANGE2_END`=1000, `NPC_RANGE3_START`=10000, `NPC_RANGE3_END`=20000. Sprite ids from mobs (1001..~4999+) and job ids work as NPC sprites via the numeric path or AegisName.
- Data range on positioned script/shop/duplicate entries in the default manifest: min -1 (2850 entries, used for invisible trigger NPCs), max 21887; none exceed 32767. A signed 16-bit field would hold everything, but `JT_INVISIBLE` 32767 and the `-1` sentinel argue for `i32` in the converter output (or `i16` with `-1` allowed).

## 6. Counts for the default renewal manifest

Manifest resolves to 835 files (all exist; 0 missing; 3 use CRLF line endings; 0 BOMs seen).

| Kind | Positioned | Floating (`-`) |
| :--- | ---: | ---: |
| warp (incl. `(DISABLED)`) | 3876 | n/a |
| warp2 (incl. `(DISABLED)`) | 229 | n/a |
| script (incl. 163 `(CLOAKED)`, 128 `(DISABLED)`, 1 `(HIDDEN)`) | 8635 (2 have 5-field `w1`) | 584 |
| duplicate | 10377 | 121 |
| shop | 270 | 32 |
| marketshop | 0 | 45 |
| cashshop / itemshop / pointshop | 0 | 0 |
| function (not an NPC) | n/a | 149 |
| monster / boss_monster (skip) | 3814 / 75 | n/a |
| mapflag (skip) | 5805 | n/a |

- Total positioned NPC headers (warp+warp2+script+duplicate+shop): 23,387 across 838 distinct map names (map names include instance maps like `1@mcd`).
- Duplicate sources by subtype (all duplicates incl. floating): script 10441, warp2 44, shop 13. Every `duplicate(src)` source is defined earlier in load order (0 forward references).
- Distinct sprite tokens on positioned script/shop/duplicate entries: 757 distinct names/aliases (705 `JT_` constant names, 51 mob AegisNames, 1 empty) plus 860 distinct numeric tokens.
- Unclassified lines: 0. Odd but accepted: `rgsr_in,135,186, 7` (space before dir), `1@mcd,144,212,5,0` (5 fields), dirs 8/15/55, and sprite tokens with a trailing `//` comment or a leading extra tab.
- Barters (`npc/barters.yml`) add more positioned NPCs separately and are not counted here.
