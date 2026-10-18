# Base and Derived Stat Formulas

Target client version: `PACKETVER = 20260219`.

## Derived Stat Formulas

* **Provenance:** `rathena` (`src/map/status.cpp`, `src/map/status.hpp`) & `rust-ro` (`tools/stat-calc/`), `PACKETVER = 20260219` (Renewal mode).
* **Scope:** Pure stateless mathematical functions computing derived combat attributes (`StatusATK`, `WeaponATK`, `StatusMATK`, `HardDEF`, `SoftDEF`, `HardMDEF`, `SoftMDEF`, `HIT`, `FLEE`, `CRI`, `ASPD`, `MaxHP`, `MaxSP`) from primary attributes (`STR`, `AGI`, `VIT`, `INT`, `DEX`, `LUK`), base level, job level, and equipment bonuses.
* **Catalog Status:** Initialized placeholder. Detailed formula tables are populated by `reference-curator` as `reference-explorer` extracts Renewal equations from `src/map/status.cpp`.
