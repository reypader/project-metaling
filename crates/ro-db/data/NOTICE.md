# rAthena-derived data: origin and license

Everything under `crates/ro-db/data/` is a **modified copy of data from rAthena**
(<https://github.com/rathena/rathena>).

- **Copyright:** the rAthena Dev Teams and contributors (see rAthena's `AUTHORS` and the
  changelogs in its source files).
- **License:** GNU General Public License v3.0 (`GPL-3.0-only`), the license rAthena uses.
  The full text is in [`LICENSE`](./LICENSE). This directory is the only GPL-licensed part of
  Project Metaling. Everything else is MIT (see the repository README).
- **Use:** the servers read these files from disk at runtime. They are never compiled into a
  binary or included in a crate package (`crates/ro-db/Cargo.toml` excludes `data/`).

Every generated file starts with an SPDX tag and a short attribution comment naming its rAthena
commit. Each dataset below lists its source, the changes made, and how to regenerate it. When a
dataset is regenerated from a different commit, update its section.

## NPC spawn lists (`npc/<map>.yml`)

- **Source:** rAthena commit `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39`. The NPC source files
  under `npc/` and `npc/re/` that the renewal manifest `npc/re/scripts_main.conf` loads.
- **Modified on 2026-10-22 by `tools/npc-import`** (regenerated 2026-10-23 as YAML 1.2): converted
  from tab-delimited `.txt` to per-map YAML (`NPC_SPAWN_DB` version 1). The conversion:
  - keeps only the headers of NPCs placed on a map (`warp`, `warp2`, `script`, `duplicate(...)`,
    and the shop keywords);
  - drops script bodies, shop item lists, floating NPCs, functions and mapflags (monster spawn
    lines go to the mob spawn lists below);
  - resolves sprite names (e.g. `4_F_KAFRA1`) to numeric class ids;
  - resolves each `duplicate(...)`'s source (kind, state, warp destination);
  - splits `display::unique` names into two fields;
  - groups entries by map in rAthena load order. Each entry records its original
    `<file>:<line>` in `Source`.
- **Regenerate** (writes this dataset and the mob spawn lists in one run; pass an expanded path,
  not one starting with `~`):

  ```bash
  cargo run -p npc-import -- --rathena "$RATHENA_PATH" \
    --out crates/ro-db/data/npc --mob-out crates/ro-db/data/mob_spawn
  ```

## Mob spawn lists (`mob_spawn/<map>.yml`)

- **Source:** rAthena commit `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39`. The permanent spawn lines
  (`monster` / `boss_monster`) in the NPC source files that the renewal manifest
  `npc/re/scripts_main.conf` loads, plus the mob names in `db/re/mob_db.yml`.
- **Modified on 2026-10-23 by `tools/npc-import`:** converted from tab-delimited `.txt` to per-map
  YAML (`MOB_SPAWN_DB` version 1). The conversion:
  - resolves mob `AegisName` references to numeric mob ids;
  - replaces the `--en--` / `--ja--` name tokens with the mob_db `Name` / `JapaneseName`;
  - cuts names to 23 characters, and sets a negative spawn span to 0;
  - strips the quotes around event labels, and drops event values shorter than 4 characters
    (`0`, `1`);
  - writes the default `Delay1` (5000) where a line has none, and omits the other defaults;
  - groups entries by map in rAthena load order. Each entry records its original
    `<file>:<line>` in `Source`.
- **Regenerate:** the same command as the NPC spawn lists above.
