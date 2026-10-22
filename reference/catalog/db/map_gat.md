# Map Walkability (`.gat` / `.rsw`) and Spawn Formats

Target client version: `PACKETVER = 20260219`.

## Map Walkability and Spawn Formats

* **Provenance:** `rathena` (`src/map/map.cpp`, `src/map/npc.cpp`, `npc/re/warps/`, `npc/re/mobs/`), `PACKETVER = 20260219`.
* **`.gat` Ground Altitude Format:**
  * Magic Header: `6` bytes (`"GRAT\x01\x02"`). rAthena does NOT validate the magic; it reads sizes at fixed offsets (`map/map.cpp:3868-3869`).
  * Dimensions: `width` (`i32` little-endian) at offset `6`, `height` (`i32` little-endian) at offset `10`.
  * Cell Array: `width * height` contiguous 20-byte records starting at offset `14`, row-major with index `x + y*width` (`map.cpp:3876-3887`). Each record has four `f32` corner altitudes (`16` bytes) followed by a `u32` `cell_type` at record offset `16`. See [Cell type mapping](#gat-cell-type-mapping) for semantics.
* **Warp and Monster Spawn Definitions:**
  * Warp definitions in `npc/re/warps/` declare source map, `(x, y)`, trigger span `(xs, ys)`, and destination map with target `(x, y)`.
  * Monster spawn definitions in `npc/re/mobs/` declare map name, spawn coordinates or bounding box, `mob_id`, spawn count, and respawn delay timers.

## `.gat` Cell Type Mapping

* **Provenance:** `rathena` `src/map/map.cpp:3280-3310` (`map_gat2cell`, `map_cell2gat`), `3855-3893` (`map_readgat`), `3315-3395` (`map_getcellp`), `3672-3717` (`map_readfromcache`). Not packet-version dependent.
* rAthena stores each cell as three bits: `walkable`, `shootable`, `water`. The raw `u32` is mapped (`map_gat2cell`):

| Raw type | walkable | shootable | water | Note |
| :--- | :--- | :--- | :--- | :--- |
| 0 | yes | yes | no | walkable ground |
| 1 | no | no | no | wall |
| 2 | yes | yes | no | unknown in source ("???"), treated as plain ground |
| 3 | yes | yes | yes | walkable water |
| 4 | yes | yes | no | unknown ("???"), plain ground |
| 5 | no | yes | no | gap/cliff: not walkable but projectiles/skills pass ("snipable") |
| 6 | yes | yes | no | unknown ("???"), plain ground |
| anything else | no | no | no | `ShowWarning("unrecognized gat type")`; all-zero cell = wall |

* Water promotion at load: a raw `0` cell whose altitude `height` (first `f32` of the record, the top-left corner) is greater than the `.rsw` water height (`map_waterheight`) becomes type `3` (`map.cpp:3884-3885`). Only the first corner altitude is read; the other three are ignored. Maps with `RSW_NO_WATER` skip this. `water_height` is an `i32` from `grfio_read_rsw_water_level` on `data/<map>.rsw` (`map.cpp:3837-3850`); the comparison is literally `height > water_height` (`f32` vs `i32`), so mirror it exactly (do not flip the sign).
* Query semantics (`map_getcellp`, `map.cpp:3323-3395`): `CHKPASS`/`CHKREACH` = `walkable`; `CHKNOPASS`/`CHKNOREACH` = `!walkable`; `CHKWALL` = `!walkable && !shootable`; `CHKCLIFF` = `!walkable && shootable`; `CHKWATER` = `water`. Shootability (line of sight, `path_search_long`) uses `CHKNOPASS` in the movement path but ranged skills use `CELL_CHKWALL`-style checks elsewhere.
* Bounds quirk: any `x < 0 || x >= xs-1 || y < 0 || y >= ys-1` returns "blocked" for `CHKNOPASS` (and 0 for every other query), "intentionally overriding the last row and column" (`map.cpp:3329-3331`). The last column and last row of every map are therefore never walkable, regardless of the raw type.
* map_cache variant (`db/map_cache.dat`, produced by rAthena's mapcache tool): zlib-compressed per-map array of one byte per cell with the same raw type values (post water promotion), header `file_size:u32, map_count:u16`, per-map `name[MAP_NAME_LENGTH], xs:i16, ys:i16, len:i32` (`map.cpp:156-167, 3672-3717`). Metaling reads `.gat` directly, so only the raw type table above matters.
