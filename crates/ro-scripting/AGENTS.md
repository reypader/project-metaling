# `crates/ro-scripting` Module Rules (NPC & Item Script VM Adapter)

## 1. Module Scope & Operational Guidelines
* **Role & Scope:** Governs the `rathena-script-lang-interpreter` VM adapter, non-blocking NPC dialog suspension/resumption, and item scripts under `./crates/ro-scripting/**`.
* **Design Reference:** Read `./crates/ro-scripting/DESIGN.md` for architectural design direction.
* **Direct Inspection:** Agents working on `./crates/ro-scripting/**` inspect engine callers (`ro-engine`), contracts, and reference catalogs directly from disk without messaging barriers.
* **Contract Compliance:** Inspect `./crates/ro-contracts/src/**` directly from disk. If a shared contract requires modification, update it according to task requirements and verify with `cargo check -p ro-contracts`.
* **Reference Access:** Consult `./reference/catalog/` or the `reference-explorer` subagent whenever script VM hooks, native function bindings, or NPC script syntax are required.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-scripting` requires invoking `code-reviewer` to audit VM continuation safety and edge cases before completion.
* **Verification Command:** Run `cargo test -p ro-scripting` to verify compilation and test passage.
* **Scripting Delegation:** Delegate one-off script syntax parsers or test script converters to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.

---

## 2. Script VM Architecture & Non-Blocking Execution

`crates/ro-scripting` adapts the rAthena script language interpreter to execute NPC dialogue scripts, warp scripts, and item bonus scripts in a non-blocking manner compatible with the synchronous ECS tick loop.

### 2.1 Non-Blocking Script Suspension & Resumption
1. **Yield-on-Interaction Model:**
   * rAthena NPC scripts use imperative builtins (`mes "text";`, `next;`, `select("opt1:opt2");`, `input @var;`, `close;`, `warp "map", x, y;`).
   * Because `ro-engine` executes a deterministic synchronous tick loop, script execution must never block the calling thread waiting for client network packets.
   * When a script invokes an interactive builtin (`next`, `select`, `input`, `close`), `ro-scripting` captures the VM continuation frame state associated with the player's `CharId` and yields a script action DTO (`DisplayDialog`, `WaitNext`, `PromptMenu`, `CloseDialog`, `WarpPlayer`) to the caller.
2. **Resumption on Player Intent:**
   * When the player sends a dialog response packet (`CZ_REQ_NEXT_SCRIPT`, `CZ_CHOOSE_MENU`, `CZ_INPUT_EDITDLG`), the host passes the player's selection into `ro-scripting` to resume the suspended VM instance until the next yield point or script termination.
3. **Item Bonus Scripts:**
   * Evaluates stateless item equip/unequip and consumable scripts (`bonus bStr, 2;`, `itemheal 50, 0;`) into structured stat modifier or effect commands defined in `crates/ro-contracts`.

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Upstream Caller:** `crates/ro-engine` or `bins/map-server` invokes `ro-scripting` via script runner traits in `crates/ro-contracts` when a player interacts with an NPC entity, steps on a scripted portal, or uses/equips an item.
* **Contract Interface:** All native function hooks (`mes`, `next`, `select`, `warp`, `getitem`, `heal`) emit pure command and event enums defined in `crates/ro-contracts`. Direct mutation of ECS components or network sockets is prohibited.
* **Isolation Invariant:** `crates/ro-scripting` has zero dependency on `ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, or `ro-persistence`.
