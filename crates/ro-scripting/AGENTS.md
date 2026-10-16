# `crates/ro-scripting` Module Rules (NPC & Item Script VM Adapter)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-scripting.md` (`ro-scripting` custom subagent).
* **Write Scope:** `./crates/ro-scripting/src/**` and `./crates/ro-scripting/Cargo.toml` exclusively.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever script VM hooks, native function bindings, or NPC script syntax from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate with `ro-engine` and `ro-bins` via subagent messaging when integrating script execution with ECS interactions.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-scripting` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit VM continuation safety and edge cases before completion.

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
