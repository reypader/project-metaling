# Renewal YAML Database Schemas

Target client version: `PACKETVER = 20260219`.

## Renewal YAML Database Schemas

* **Provenance:** `rathena` (`db/re/item_db.yml`, `db/re/mob_db.yml`, `db/re/skill_db.yml`, `db/re/job_stats.yml`), `PACKETVER = 20260219` (Renewal mode).
* **Structure:** Every rAthena YAML database file contains a `Header` section (`Type` and `Version`) and a `Body` sequence of typed entity records, plus an optional `Footer` list of imported file paths.
* **Item Identifiers:** Item IDs (`Id` in `item_db.yml`) use 32-bit unsigned integers (`u32`) to accommodate modern `PACKETVER` values exceeding `65535`.
* **Catalog Status:** Initialized overview. Detailed field tables for `item_db`, `mob_db`, `skill_db`, from schema definitions from `env(RATHENA_PATH)/db/re/`.
