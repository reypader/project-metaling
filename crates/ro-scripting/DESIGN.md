# Project Metaling — Dual-VM Scripting & Coroutine Design (`crates/scripting` / `ro-scripting`)

## 1. Architectural Overview & Dual-Backend Philosophy
`ro-scripting` (`crates/scripting`) is the asynchronous NPC dialog, quest progression, and event scripting subsystem for **Project Metaling**.

### 1.1 Why Decouple `ro-engine` from the Scripting Language?
In legacy rAthena and Hercules, the custom C-like Athena Script bytecode VM (`script.cpp`) is tightly coupled to `map_session_data` and `npc_data` pointers. In **Project Metaling**:
1. `ro-engine` (`MapWorld`) has **zero knowledge** of Athena Script or Lua syntax. It stores only an opaque `ScriptId(pub u32)` on `EntityKind::Npc` entities.
2. `ro-scripting` implements a **Backend-Agnostic Script Runner** capable of dispatching any `ScriptId` to either:
   * **Backend A (`AthenaVmBackend`):** Wraps and adapts `reference/rathena-script-lang-interpreter` to execute unmodified official rAthena `.txt` quest and episode scripts.
   * **Backend B (`LuaVmBackend` via `mlua`):** Runs modern **Lua 5.4 / Luau** scripts using native Rust `async`/`await` coroutines (`Lua::create_async_function`), enabling clean, strongly-typed custom NPCs, instances, and boss AI scripts.

~~~text
┌──────────────────────────────────────────────────────────────────────────┐
│ crates/engine (MapWorld Stage 2 / Stage 3)                               │
│  • Player clicks NPC (`CZ_CONTACTNPC`) or steps into `OnTouch` box       │
│  • Emits `ScriptTriggerEvent { char_id, entity, npc_entity, script_id }` │
└────────────────────────────────────┬─────────────────────────────────────┘
                                     │
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ crates/scripting (ScriptHost & ActiveDialogRegistry)                     │
│  Routes `ScriptId` by backend tag:                                       │
│   ├─► [AthenaVmBackend] (`rathena-script-lang-interpreter` VM thread/fiber)│
│   └─► [LuaVmBackend]    (`mlua` async coroutine: `npc:say()`, `npc:next()`)│
│                                                                          │
│  Executes until hitting a Yield Point (`next`, `select`, `sleep2`, `end`)│
└───────────────┬──────────────────────────────────────────┬───────────────┘
                │ (Dialog UI Packets)                      │ (World Mutations)
                ▼                                          ▼
     OutboundPacket (SelfOnly)                    ScriptWorldCommand
  • `ZC_SAY_DIALOG`   (`0x00B4`)              • `WarpPlayer { map, x, y }`
  • `ZC_WAIT_DIALOG`  (`0x00B5`)              • `ModifyZeny / GiveItem`
  • `ZC_CLOSE_DIALOG` (`0x00B6`)              • `SetCharVar (sets DIRTY_BLOBS)`
  • `ZC_MENU_LIST`    (`0x00B7`)              • `SpawnDynamicMob`
~~~

---

## 2. Non-Blocking Coroutine Lifecycle (Dialog State Machine)

NPC scripts are inherently conversational and stateful: a script sends lines of text, pauses for several seconds while the player reads and clicks **"Next"** or selects a menu option, and then resumes right where it left off.

**Strict Rule:** A script suspension MUST NEVER block an ECS worker thread or hold a borrow on `MapWorld`.

### 2.1 The 4-Phase Suspension & Resumption Cycle
Each active player-NPC conversation is managed as an isolated `DialogSession` indexed by `CharId` inside `ro-scripting`:

1. **Initiation (`ScriptTriggerEvent::ContactNpc` / `OnTouch`):**
   * `ro-scripting` checks if `char_id` already has an active `DialogSession`. (If so, the click is ignored unless the current session is in a cancellable state).
   * Spawns a new VM fiber (Athena VM instance or Lua `mlua::Thread` coroutine) bound to `(char_id, player_entity, npc_entity)`.
2. **Step Execution & Output Accumulation:**
   * Non-yielding commands (`mes "Hello";`, math expressions, variable reads) accumulate into the session's outbound buffer without pausing.
3. **Suspension at Yield Boundaries (`DialogYieldState`):**
   * When the script hits a blocking built-in, the coroutine suspends and sets the session's expected resume token:
     * `next;` / `npc:next()` $\rightarrow$ Emits buffered `ZC_SAY_DIALOG` + `ZC_WAIT_DIALOG` (`0x00B5`), transitions to `AwaitingNext`.
     * `select("Opt A:Opt B");` / `npc:select({...})` $\rightarrow$ Emits `ZC_MENU_LIST` (`0x00B7`), transitions to `AwaitingMenu { option_count }`.
     * `input .@val;` / `npc:input()` $\rightarrow$ Emits `ZC_OPEN_EDITDLG` (`0x0142` / `0x01D4`), transitions to `AwaitingInput`.
     * `sleep2 1000;` / `npc:sleep_ms(1000)` $\rightarrow$ Registers a wakeup tick (`current_tick + ticks`), transitions to `Sleeping { wake_tick }`.
     * `close;` / `end;` $\rightarrow$ Emits `ZC_CLOSE_DIALOG` (`0x00B6`) (for `close;`) and drops the `DialogSession`.
4. **Resumption or Safe Cancellation (`ScriptResumeEvent`):**
   * When the client responds with `CZ_REQ_NEXT_SCRIPT` (`0x00B9`), `CZ_CHOOSE_MENU` (`0x00B8`), or `CZ_INPUT_EDITDLG`, `ro-engine` forwards `ScriptResumeEvent` to `ro-scripting`.
   * **Protocol Validation:** `ro-scripting` verifies that the incoming resume variant matches the session's current `DialogYieldState` (preventing packet-injection skips where a client sends `CZ_REQ_NEXT_SCRIPT` when a menu selection is required).
   * **Cancel / Disconnect Cleanup:** If the player selects `0xFF` (Cancel) on a menu, logs out, dies, or is warped away while a dialog is suspended, `ro-scripting` immediately aborts and drops the `DialogSession` fiber—eliminating rAthena's dangling `RID` bugs.

---

## 3. Variable Scoping & Persistence Bridge (`ro-persistence` Integration)

The Athena scripting language relies on **variable name prefixes** to determine both memory lifetime and database persistence. `ro-scripting` unifies all six scopes across both the Athena VM and Lua (via Lua proxy metatables):

| Athena Prefix | Lua Equivalent (`ctx` API) | Scope & Lifetime | Storage & Persistence Backend |
| :--- | :--- | :--- | :--- |
| **`.@var`** | `local var = ...` | **Scope-Local Temporary:** Freed as soon as the script/coroutine ends. | Stored directly on the VM execution stack. Never leaves `ro-scripting`. |
| **`@var`** | `ctx.temp.var` | **Character Temporary:** Lives for the player's online session; cleared on logout. | Stored in an in-memory `FxHashMap<String, ScriptValue>` attached to the player session. |
| **`var`** *(no prefix)* | `ctx.char.var` | **Character Permanent:** Quest progress & flags persisted across logins. | Cached on the player's `ScriptVars` component. Any write flips `PersistenceDirtyFlags::DIRTY_BLOBS` (`1 << 1`), flushing to PostgreSQL `character_blobs.script_vars` (`JSONB`). |
| **`#var` / `##var`** | `ctx.account.var` | **Account Permanent:** Shared across all characters on the same `AccountId`. | Flushed via `ro-persistence` to account-level `JSONB` storage. |
| **`$var`** | `ctx.world.var` | **Server-Wide Permanent:** Global event states, WoE flags, server counters. | Stored in a shared `DashMap<String, ScriptValue>` and flushed asynchronously to PostgreSQL `mapreg`. |
| **`.var`** | `ctx.npc.var` | **NPC-Instance Static:** Shared by all players talking to that specific NPC entity. | Stored inside `ro-scripting` indexed by `(NpcEntity, String)`. Reset on `@reloadscript` / server reboot. |

*Note on Postfix Type Sigils (`$`):* Both integer (`JobQuest = 1`) and string (`JobQuest$ = "Novice"`) values are stored as a tagged enum `ScriptValue { Int(i64), Str(String) }` serialized cleanly into `character_blobs.script_vars` (`JSONB`).

---

## 4. World Commands & Event Label Dispatch (`ro-contracts`)

Because `ro-scripting` cannot mutate `MapWorld` ECS columns (`Vitals`, `Inventory`, `MapPosition`) directly, all side effects produced by built-in script commands (`warp`, `getitem`, `delitem`, `heal`, `monster`, `sc_start`) are emitted as buffered `ScriptWorldCommand` messages defined in `crates/contracts`:

~~~rust
use crate::{CharId, Entity, ItemId, MapPosition, MobId, ScriptId};

/// Inbound triggers sent from `ro-engine` (`MapWorld`) to `ro-scripting`.
#[derive(Debug, Clone)]
pub enum ScriptTriggerEvent {
    /// Fired once at server boot for every script defining an `OnInit:` label/hook.
    ServerInit,
    /// Player clicked an NPC (`CZ_CONTACTNPC` - 0x0090).
    ContactNpc {
        char_id: CharId,
        player_entity: Entity,
        npc_entity: Entity,
        script_id: ScriptId,
    },
    /// Player stepped into an NPC's `(xs, ys)` `OnTouch:` proximity area in Stage 3.
    TouchNpc {
        char_id: CharId,
        player_entity: Entity,
        npc_entity: Entity,
        script_id: ScriptId,
    },
    /// Player responded to an open dialog window (`0x00B9`, `0x00B8`, `0x0142`, `0x01D5`).
    ResumeDialog {
        char_id: CharId,
        input: DialogResumeInput,
    },
    /// Named event hook (e.g., `"NpcName::OnMyMobDead"`, `"OnPCDieEvent"`, `"OnClock0000"`).
    NamedEvent {
        event_label: String,
        attached_char: Option<CharId>,
    },
    /// Aborts any suspended coroutine for `char_id` on logout, death, or unexpected warp.
    CancelDialog {
        char_id: CharId,
    },
}

#[derive(Debug, Clone)]
pub enum DialogResumeInput {
    Next,
    MenuChoice(u8),      // 1-based index; 0xFF = Cancel
    NumberInput(i32),
    StringInput(String),
}

/// Outbound mutations emitted by `ro-scripting` back to `ro-engine` (`MapWorld`).
#[derive(Debug, Clone)]
pub enum ScriptWorldCommand {
    /// Emits `ZC_SAY_DIALOG`, `ZC_WAIT_DIALOG`, `ZC_MENU_LIST`, or `ZC_CLOSE_DIALOG` to `char_id`.
    SendDialogPacket {
        char_id: CharId,
        packet: DialogPacketPayload,
    },
    /// Teleports player (`warp "map", x, y;` or Kafra teleport service).
    WarpPlayer {
        char_id: CharId,
        dest_map: String,
        dest_pos: (u16, u16),
    },
    /// Modifies player HP/SP (`heal hp, sp;` or `percentheal`).
    ModifyVitals {
        char_id: CharId,
        hp_delta: i32,
        sp_delta: i32,
    },
    /// Modifies player Zeny and/or inventory (`getitem`, `delitem`, `Zeny -= 500`).
    ModifyEconomy {
        char_id: CharId,
        zeny_delta: i64,
        items_added: Vec<(ItemId, u16)>,
        items_removed: Vec<(ItemId, u16)>,
    },
    /// Mutates a permanent character variable (`var`) and sets `DIRTY_BLOBS`.
    SetCharPermanentVar {
        char_id: CharId,
        key: String,
        value: ScriptValue,
    },
    /// Dynamically spawns monsters on a map (`monster "map", x, y, "name", mob_id, amount;`).
    SpawnDynamicMob {
        map_name: String,
        pos: Option<(u16, u16)>,
        mob_id: MobId,
        count: u16,
        on_death_event: Option<String>,
    },
    /// Enables or disables an NPC entity (`enablenpc` / `disablenpc`).
    SetNpcEnabled {
        unique_name: String,
        enabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScriptValue {
    Int(i64),
    Str(String),
}
~~~

---

## 5. Pre-Check Guards for Transactional Commands (`getitem` / `Zeny`)

When a script executes an economy command like `if (Zeny >= 500) { Zeny -= 500; getitem 501, 1; }`, `ro-scripting` requires synchronous read access to the player's current Zeny, Weight, and Inventory counts without holding a lock on `MapWorld`:
1. **Read Snapshot on Dialog Start (`PlayerScriptSnapshot`):** When `ro-engine` emits `ScriptTriggerEvent::ContactNpc`, it attaches a lightweight copy of the player's current scalar state (`zeny`, `base_level`, `job_class`, `current_weight`, `max_weight`, and inventory item counts) plus a read handle to the player's `ScriptVars`.
2. **Optimistic Validation & Atomic Commit:** The script validates `Zeny >= 500` and weight limits against its local `PlayerScriptSnapshot`, updates the snapshot in-place for subsequent lines in the same script step, and emits `ScriptWorldCommand::ModifyEconomy` so `ro-engine` commits the authoritative delta during Stage 4.