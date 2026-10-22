# NPC Dialog and Navigation Builtins

Target client version: `PACKETVER = 20260219`.

## NPC Dialog and Navigation Builtins

* **Provenance:** `rathena` (`src/map/script.cpp`, `doc/script_commands.txt`) & `rathena-script-lang-interpreter` (`src/`), `PACKETVER = 20260219`.
* **Scope:** Native script VM hooks and suspension states for interactive NPC conversations:
  * `mes`: Appends a dialog text line to the active client conversation window.
  * `next`: Suspends script execution until the player clicks the Next button.
  * `close`: Appends a Close button to the dialog window and terminates conversation state upon acknowledgement.
  * `select` / `menu`: Suspends script execution to display a 1-indexed choice menu and resumes with the selected index.
  * `warp`: Teleports the interacting character to a target map name and `(x, y)` coordinate pair.
* **Catalog Status:** Initialized overview. Detailed VM hook signatures and packet sequences are extracts specifications from `rathena-script-lang-interpreter` and `src/map/script.cpp`.
