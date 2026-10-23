# Project Metaling

A clean-room Ragnarok Online server written in Rust, built on a custom ECS. It targets a single
client version, `PACKETVER = 20260219`.

| Path | What it is |
| :--- | :--- |
| `bins/{login,char,map}-server` | The three server processes |
| `crates/ro-core` | Shared IDs, DTOs, client intents and server events |
| `crates/ro-packets` | Wire framing, packet parsing and serialization |
| `crates/ro-engine` | Per-map ECS runtime: ticks, movement, area of interest |
| `crates/ro-db` | Static data: `.gat` walkability, NPC and mob spawn lists, registries |
| `crates/ro-persistence` | PostgreSQL storage |
| `tools/proxy` | Packet-logging proxy (dev only) |
| `tools/npc-import` | Converts rAthena NPC files to the YAML in `crates/ro-db/data/npc` and `crates/ro-db/data/mob_spawn` (dev only) |

`./verify.sh` runs fmt, clippy and the tests. `CLAUDE.md` has the full set of commands.

## License

This repository is **mixed-license**:

- **Code and documentation:** MIT. See [`LICENSE`](./LICENSE).
- **`crates/ro-db/data/`:** GPL-3.0. This is game data derived from rAthena: NPC spawns today,
  and later monsters, items, skills and scripts. It carries its own
  [`LICENSE`](./crates/ro-db/data/LICENSE) and [`NOTICE.md`](./crates/ro-db/data/NOTICE.md),
  which describe each dataset's origin and the changes made. The servers read these files from
  disk at runtime; they are not compiled into any binary or crate package.

No code is copied from the reference projects below. Metaling is written from our own notes in
`reference/catalog/`.

## Acknowledgements

- **[rAthena](https://github.com/rathena/rathena)** (GPL-3.0), by the rAthena Dev Teams and
  contributors. It is the reference implementation for protocol and game behavior, and the source
  of the data in `crates/ro-db/data/`.
- **[rust-ro](https://github.com/nmeylan/rust-ro)** (MIT), by Nicolas Meylan. It was a
  reference for a Rust take on the server.
- **[rathena-script-lang-interpreter](https://github.com/nmeylan/rathena-script-lang-interpreter)**,
  by Nicolas Meylan. It was a reference for rAthena script semantics. It has no license, so we only
  read it and never copy from it.
