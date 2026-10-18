# Map Walkability (`.gat` / `.rsw`) and Spawn Formats

Target client version: `PACKETVER = 20260219`.

## Map Walkability and Spawn Formats

* **Provenance:** `rathena` (`src/map/map.cpp`, `src/map/npc.cpp`, `npc/re/warps/`, `npc/re/mobs/`) & `rust-ro` (`tools/map-cache/`), `PACKETVER = 20260219`.
* **`.gat` Ground Altitude Format:**
  * Magic Header: `6` bytes (`"GRAT\x01\x02"`).
  * Dimensions: `width` (`u32` little-endian) at offset `6`, `height` (`u32` little-endian) at offset `10`.
  * Cell Array: `width * height` contiguous 20-byte records starting at offset `14`. Each record contains four `f32` corner altitudes (`16` bytes) followed by a `u32` `cell_type` field (`0` = walkable ground, `1` = non-walkable wall, `3` = walkable water, `5` = cliff/shootable non-walkable).
* **Warp and Monster Spawn Definitions:**
  * Warp definitions in `npc/re/warps/` declare source map, `(x, y)`, trigger span `(xs, ys)`, and destination map with target `(x, y)`.
  * Monster spawn definitions in `npc/re/mobs/` declare map name, spawn coordinates or bounding box, `mob_id`, spawn count, and respawn delay timers.
