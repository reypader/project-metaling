# Status Change Lifecycles and Timers

Target client version: `PACKETVER = 20260219`.

## Status Change Lifecycle

* **Provenance:** `rathena` (`src/map/status.cpp`, `src/map/status.hpp`, `db/re/status.yml`), `PACKETVER = 20260219` (Renewal mode).
* **Scope:** Status effect application, resistance checks, duration timers, periodic tick intervals, and attribute overrides for buffs and debuffs (`sc_start`, `sc_end`, `SC_*` constants).
* **Catalog Status:** Initialized placeholder. Detailed status definitions are populated by `reference-curator` as `reference-explorer` extracts specifications from `src/map/status.cpp` and `db/re/status.yml`.
