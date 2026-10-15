# AGENTS.md — `crates/persistence` Subagent (PostgreSQL & Repositories)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/persistence/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/persistence/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`.

## 2. Responsibilities
1. Implement the `AccountRepository` and `CharacterRepository` async traits defined in `crates/contracts`.
2. Provide an `InMemoryStubRepo` for Phases 0–3 (returning a hardcoded test account and character at `prontera` `(156, 191)`).
3. Provide a `PostgresRepo` using `sqlx` for Phase 4+, managing SQL migrations for accounts and characters (location, hair style, hair color, sex, slot, stats). Reference `reference/rathena/sql-files/main.sql` and `reference/rust-ro/server/src/repository/`.
