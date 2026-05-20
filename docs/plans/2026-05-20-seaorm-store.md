# SeaORM Store Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move relational market-data persistence from handwritten SQLx calls to SeaORM while keeping SQLite tests and allowing database selection by URL.

**Architecture:** `fastquote-store` owns SeaORM entities and an ORM-backed store with the same write behavior as the current SQLite store. `Persister` depends on the generic database store, and `fastquote-bin` prefers `store.database_url` with `sqlite_url` as a compatibility fallback.

**Tech Stack:** Rust, tokio, SeaORM, SeaQuery, SQLite in-memory tests, TOML config.

---

### Task 1: Add Config Regression Tests

**Files:**
- Modify: `fastquote-bin/src/config.rs`

**Step 1: Write failing tests**

Add tests proving `database_url` is accepted and `sqlite_url` remains a fallback.

**Step 2: Run tests**

Run: `CARGO_HOME=/private/tmp/codex-cargo-home cargo test -p fastquote-bin config -- --nocapture`

Expected: fail because `StoreSection` has no `database_url` handling yet.

**Step 3: Implement config support**

Add optional `database_url` and helper method returning `database_url` or
`sqlite_url`.

**Step 4: Run tests**

Run the same command and expect pass.

### Task 2: Add ORM Store Compatibility Tests

**Files:**
- Modify: `fastquote-store/tests/sqlite_store.rs`

**Step 1: Write failing test**

Update the tests to instantiate the generic ORM-backed store alias and assert
the same persistence/upsert behavior.

**Step 2: Run tests**

Run: `CARGO_HOME=/private/tmp/codex-cargo-home cargo test -p fastquote-store sqlite_store -- --nocapture`

Expected: fail because the ORM-backed store does not exist yet.

### Task 3: Implement SeaORM Entities And Store

**Files:**
- Modify: `Cargo.toml`
- Modify: `fastquote-store/Cargo.toml`
- Create: `fastquote-store/src/entity/mod.rs`
- Create: `fastquote-store/src/entity/depth_tick.rs`
- Create: `fastquote-store/src/entity/kline.rs`
- Create: `fastquote-store/src/entity/index_quote.rs`
- Modify: `fastquote-store/src/sqlite_store.rs`
- Modify: `fastquote-store/src/lib.rs`

**Step 1: Add dependencies**

Add SeaORM with SQLite, MySQL, PostgreSQL, tokio rustls runtime, macros, and
schema support.

**Step 2: Define entities**

Represent the three existing tables with composite primary keys and no
relations.

**Step 3: Implement schema initialization**

Use SeaORM schema builder to create tables and indexes if they do not exist.

**Step 4: Implement writes**

Map market data into active models and use SeaORM insert with conflict update on
the composite primary keys.

**Step 5: Run store tests**

Run: `CARGO_HOME=/private/tmp/codex-cargo-home cargo test -p fastquote-store sqlite_store -- --nocapture`

Expected: pass.

### Task 4: Rename Persister Database Field

**Files:**
- Modify: `fastquote-store/src/persister.rs`
- Modify: `fastquote-bin/src/main.rs`

**Step 1: Update naming**

Rename internal `sqlite` field and logs to database-oriented names. Pass
`config.store.database_url()` to `Persister::new`.

**Step 2: Run targeted tests**

Run:

```bash
CARGO_HOME=/private/tmp/codex-cargo-home cargo test -p fastquote-store -p fastquote-bin -- --nocapture
```

Expected: pass.

### Task 5: Full Verification

Run:

```bash
CARGO_HOME=/private/tmp/codex-cargo-home cargo test --workspace -- --nocapture
```

Expected: all workspace tests pass.
