# NPC spawn data: origin and license

The `*.yml` files in this directory are a **modified copy of data from rAthena**
(https://github.com/rathena/rathena), commit `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39`.
They come from the NPC source files under `npc/` and `npc/re/` that rAthena's renewal manifest
`npc/re/scripts_main.conf` loads.

- **Copyright:** the rAthena Dev Teams and contributors (see rAthena's `AUTHORS` and the
  per-file changelogs in its `npc/` tree).
- **License:** GNU General Public License v3.0 (`GPL-3.0-only`), the same license rAthena uses.
  The full text is in [`LICENSE`](./LICENSE) in this directory. This directory is the only
  GPL-licensed part of Project Metaling. Everything else is MIT (see the repository README).

## Modifications

On 2026-10-22, Project Metaling's `tools/npc-import` converted this data from rAthena's
tab-delimited `.txt` format into per-map YAML (`NPC_SPAWN_DB` version 1). The conversion:

- keeps only the headers of NPCs placed on a map (`warp`, `warp2`, `script`, `duplicate(...)`,
  and the shop keywords), and drops script bodies, shop item lists, floating NPCs, functions,
  monster spawns and mapflags;
- resolves sprite names (e.g. `4_F_KAFRA1`) to numeric class ids, and resolves each
  `duplicate(...)`'s source (kind, state, warp destination);
- splits `display::unique` names into two fields;
- groups entries by map, one file per map, in rAthena load order. Each entry records its
  original `<file>:<line>` in `Source`.

Each file starts with an SPDX tag and a short attribution comment.

## Regenerating

```bash
cargo run -p npc-import -- --rathena "$RATHENA_PATH" --out crates/ro-db/data/npc
```

When you regenerate from a different rAthena commit, update the commit hash and date above.
