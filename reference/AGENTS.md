# AGENTS.md: `./reference` (Reference Repositories & Knowledge Catalog)

## 1. Directory Purpose & Ownership
The `./reference` directory houses both gitignored upstream reference repositories and the version-controlled Project Metaling knowledge catalog (`./reference/catalog/`).

* **`reference-explorer` (`.agents/agents/reference-explorer.md`):**
  * The dedicated reference exploration and knowledge curation specialist.
  * Consults `./reference/catalog/INDEX.md` and domain topic files first before traversing raw upstream repositories.
  * Extracts uncached protocol layouts, YAML schemas, SQL schemas, and Aegis mechanics from the cloned upstream repositories.
  * Authors, updates, deduplicates, and organizes specifications in `./reference/catalog/**` and maintains `./reference/sync-references.sh`.
  * Delegates complex scratch parsing scripts or bulk header transformations to `utility-scripter`.
* **Workspace Access:**
  * All agents may read `./reference/catalog/**` directly to inspect curated packet formats, schemas, and formulas.
  * When reference behavior is uncached or requires upstream exploration, agents consult `reference-explorer`.
  * Scratch scripting is delegated to `utility-scripter` and is exempt from code and architectural reviews.

## 2. Directory Layout

```text
reference/
├── AGENTS.md                        # Reference governance & catalog rules
├── sync-references.sh               # [reference-explorer] Clones or updates gitignored upstream repos
├── catalog/                         # [reference-explorer] Tracked knowledge base
│   ├── INDEX.md                     # Master lookup table mapping topics/opcodes to catalog files
│   ├── packets/                     # Wire protocol & packet layout specifications (PACKETVER = 20260219)
│   │   ├── login.md                 # Login server (CA_* / AC_*) packet layouts
│   │   ├── char.md                  # Char server (CH_* / HC_*) packet layouts
│   │   ├── map.md                   # Map server (CZ_* / ZC_*) packet layouts & shuffled opcodes
│   │   └── bitpacking.md            # WBUFPOS / WBUFPOS2 coordinate & direction bit-packing
│   ├── mechanics/                   # Stateless Aegis / Renewal math & entity lifecycles
│   │   ├── stats.md                 # Base/derived stat formulas (ATK, DEF, ASPD, HIT, FLEE)
│   │   ├── combat.md                # Damage formulas, elemental/size/race modifier tables
│   │   ├── movement_aoi.md          # Map entry handshake, walking ticks, sitting, AoI broadcast rules
│   │   └── status.md                # Status change lifecycles and timers
│   ├── scripting/                   # NPC & item script VM specifications
│   │   ├── npc.md                   # Dialog commands (mes, next, close, select, menu, warp)
│   │   ├── skills.md                # Skill script hooks and ratio tables
│   │   └── items.md                 # Item bonus script evaluation rules
│   └── db/                          # Static YAML database & SQL schema specifications
│       ├── yaml_schemas.md          # Renewal YAML schemas (item_db, mob_db, skill_db, job_db)
│       ├── sql_schemas.md           # Relational schemas (login, char, inventory)
│       └── map_gat.md               # .gat / .rsw cell walkability & warp/mob spawn file formats
├── rathena/                         # [Gitignored] https://github.com/rathena/rathena.git
├── Hercules/                        # [Gitignored] https://github.com/HerculesWS/Hercules.git
├── rust-ro/                         # [Gitignored] https://github.com/nmeylan/rust-ro.git
└── rathena-script-lang-interpreter/ # [Gitignored] https://github.com/nmeylan/rathena-script-lang-interpreter.git
```

## 3. Catalog Entry Standards
1. **Master Index First (`./reference/catalog/INDEX.md`):**
   Every cataloged opcode, schema, formula, and script command must appear in `INDEX.md` with its hex code or identifier, brief summary, and relative link to the domain file and section.
2. **Provenance Metadata:**
   Every specification section must list the upstream repository name, source file path, line numbers, and `PACKETVER = 20260219` applicability.
3. **File Size Limit:**
   Keep each catalog file under 400 lines. Split growing files into sub-topic files and update `INDEX.md`.
