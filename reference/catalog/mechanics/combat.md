# Combat Damage Formulas and Modifier Tables

Target client version: `PACKETVER = 20260219`.

## Combat Damage Pipeline

* **Provenance:** `rathena` (`src/map/battle.cpp`, `src/map/battle.hpp`, `db/re/attr_fix.yml`, `db/re/size_fix.yml`), `PACKETVER = 20260219` (Renewal mode).
* **Scope:** Stateless physical and magical damage pipelines, elemental attribute modifier matrices (`attr_fix`), weapon size penalty tables (`size_fix`), race and class card modifiers, critical hit evaluation, and defense reduction stages.
* **Catalog Status:** Initialized placeholder. Detailed stage equations and modifier tables are populated from `src/map/battle.cpp`.
