# Item Bonus Script Evaluation Rules

Target client version: `PACKETVER = 20260219`.

## Item Bonus Script Evaluation

* **Provenance:** `rathena` (`src/map/script.cpp`, `src/map/pc.cpp`, `db/re/item_db.yml`) & `rathena-script-lang-interpreter` (`src/`), `PACKETVER = 20260219`.
* **Scope:** Evaluation rules for `Script`, `OnEquip_Script`, and `OnUnequip_Script` blocks in `item_db.yml`, including `bonus`, `bonus2`, `bonus3`, `heal`, and `itemskill` builtins.
* **Catalog Status:** Initialized placeholder. Detailed builtin argument tables are populated by `reference-curator` as `reference-explorer` extracts specifications from `src/map/script.cpp` and `db/re/item_db.yml`.
