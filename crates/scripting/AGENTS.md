# AGENTS.md: `crates/scripting` Subagent (NPC & Item Script VM Adapter)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/scripting/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/scripting/src/**` and `./crates/scripting/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). If you need a new shared type or trait in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & Script VM Mechanics

`crates/scripting` (`ro-scripting`) adapts the rAthena script language interpreter (`reference/rathena-script-lang-interpreter/src/`) to execute NPC dialogue scripts, warp scripts, and item bonus scripts in a non-blocking manner compatible with the ECS tick loop.

### 2.1 Non-Blocking Script Suspension & Resumption
1. **Yield-on-Interaction Model:**
   * rAthena NPC scripts use imperative blocking builtins (`mes "text";`, `next;`, `select("opt1:opt2");`, `input @var;`, `close;`, `warp "map", x, y;`).
   * Because `ro-engine` executes a deterministic synchronous tick loop, script execution must never block the calling thread waiting for client network packets.
   * When a script invokes an interactive builtin (`next`, `select`, `input`, `close`), `ro-scripting` captures the VM continuation/frame state associated with the player's `CharId` and yields a script action DTO (`DisplayDialog`, `WaitNext`, `PromptMenu`, `CloseDialog`, `WarpPlayer`) back to the caller.
2. **Resumption on Player Intent:**
   * When the player sends a dialog response packet (`CZ_REQ_NEXT_SCRIPT`, `CZ_CHOOSE_MENU`, `CZ_INPUT_EDITDLG`), the host passes the player's selection into `ro-scripting` to resume the suspended VM instance until the next yield point or script termination.
3. **Item Bonus Scripts:**
   * Evaluates stateless item equip/unequip and consumable scripts (`bonus bStr, 2;`, `itemheal 50, 0;`) into structured stat modifier or effect commands defined in `crates/contracts`.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Caller:** `crates/engine` (or `bins/map-server`) invokes `ro-scripting` via script runner traits in `crates/contracts` when a player interacts with an NPC entity, steps on a scripted portal, or uses/equips an item.
* **Contract Boundary:** All native function hooks (`mes`, `next`, `select`, `warp`, `getitem`, `heal`) translate into pure command/event enums defined in `crates/contracts` rather than mutating ECS components or writing network packets directly.
* **Isolation Boundary:** `crates/scripting` has zero dependency on `ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, or `ro-persistence`.
