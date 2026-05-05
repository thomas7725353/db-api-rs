# SeaORM/SeaQuery Dynamic SQL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Rust runtime database access layer with SeaORM/SeaQuery, preserve DBAPI 3.3.0 compatibility, and add runtime dynamic SQL support plus a backend-only SeaQuery query DSL MVP.

**Architecture:** Introduce a focused database wrapper around SeaORM raw SQL and use it everywhere instead of RBatis. Build dynamic SQL rendering as a pure module that returns SQL plus bind values. Add a small SeaQuery DSL module after the compatibility layer is stable.

**Tech Stack:** Rust 2024, Axum 0.8, SeaORM 1.1, SeaQuery 0.32, SQLx via SeaORM, sqlparser, serde_json.

---

### Task 1: Replace Dependencies

**Files:**
- Modify: `runtime-rust/Cargo.toml`
- Modify: `runtime-rust/Cargo.lock`

- [ ] Remove `rbatis`, `rbs`, `rbdc-sqlite`, `rbdc-mysql`, and `rbdc-pg`.
- [ ] Add `sea-orm` with `sqlx-sqlite`, `sqlx-mysql`, `sqlx-postgres`, `runtime-tokio-rustls`, `with-json`, and `with-chrono`.
- [ ] Add explicit `sea-query`.
- [ ] Run `rtk cargo check` and expect compile errors from old RBatis references.

### Task 2: Add SeaORM Database Wrapper

**Files:**
- Create: `runtime-rust/src/db.rs`
- Modify: `runtime-rust/src/main.rs`

- [ ] Write tests for JDBC URL normalization for SQLite, MySQL, and Postgres.
- [ ] Implement `normalize_url(db_type, url, username, password)`.
- [ ] Implement `DbPoolManager` backed by `DashMap<String, DatabaseConnection>`.
- [ ] Implement `DbExecutor` helper functions:
  - `query_json`
  - `query_one_json`
  - `execute`
- [ ] Wire metadata database initialization through `db.rs`.

### Task 3: Port Metadata Repository

**Files:**
- Modify: `runtime-rust/src/repository.rs`
- Modify: `runtime-rust/src/handler.rs`
- Modify: `runtime-rust/src/basic_handler.rs`
- Modify: `runtime-rust/src/api_config_handler.rs`
- Modify: `runtime-rust/src/datasource_handler.rs`

- [ ] Replace every `&RBatis` parameter with `&DatabaseConnection` or wrapper type.
- [ ] Replace `exec_decode` calls with `query_json` plus serde conversion helpers.
- [ ] Replace `exec` calls with `execute`.
- [ ] Preserve repository function names so handlers stay minimally changed.
- [ ] Run `rtk cargo test` after the repository compiles.

### Task 4: Port User API Execution

**Files:**
- Modify: `runtime-rust/src/handler.rs`
- Modify: `runtime-rust/src/sql_engine.rs`

- [ ] Replace `rbs::Value` conversion with `sea_query::Value`.
- [ ] Replace datasource pool lookup from `PoolManager` to `DbPoolManager`.
- [ ] Execute query SQL via `query_json`.
- [ ] Execute DML SQL via `execute` and return `rowsAffected`.
- [ ] Keep existing token/access-log behavior unchanged.

### Task 5: Implement Runtime Dynamic SQL

**Files:**
- Create: `runtime-rust/src/dynamic_sql.rs`
- Modify: `runtime-rust/src/main.rs`
- Modify: `runtime-rust/src/handler.rs`
- Modify: `runtime-rust/src/api_config_handler.rs`

- [ ] Write failing tests for `<if>`, `<where>`, `<trim>`, `<foreach>`, `#{}`, and unsafe `${}`.
- [ ] Implement XML-like tag parsing sufficient for DBAPI templates.
- [ ] Implement condition evaluator for `param != null`, `param == null`, `param != ''`, `param == ''`, `and`, and `or`.
- [ ] Implement placeholder rendering to SQL and `Vec<Value>`.
- [ ] Implement `/apiConfig/parseDynamicSql`.
- [ ] Implement `/apiConfig/sql/execute`.
- [ ] Update `/api/{path}` to render dynamic SQL before sqlparser validation and execution.

### Task 6: Add SeaQuery DSL MVP

**Files:**
- Create: `runtime-rust/src/query_dsl.rs`
- Modify: `runtime-rust/src/main.rs`

- [ ] Write tests for `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `like`, `in`, `and`, `or`, sort, limit, offset, and count SQL generation.
- [ ] Implement JSON DSL parsing.
- [ ] Validate table and field identifiers.
- [ ] Build SeaQuery `SelectStatement`.
- [ ] Add backend endpoint for query execution against a datasource and table.
- [ ] Do not add a rewritten frontend in this task.

### Task 7: Update Demo Seed And Skills

**Files:**
- Modify: `runtime-rust/seed_demo_api.sql`
- Modify: `skills/dbapi-demo-crud/SKILL.md`
- Modify: `skills/dbapi-demo-crud/scripts/seed_demo_api.sql`

- [ ] Change Demo Item list SQL to use dynamic SQL tags.
- [ ] Add curl examples for dynamic SQL parsing and SQL editor execution.
- [ ] Keep token creation/query examples.

### Task 8: End-To-End Verification

**Files:**
- No source changes expected.

- [ ] Run `rtk cargo fmt`.
- [ ] Run `rtk cargo test`.
- [ ] Run `rtk cargo check`.
- [ ] Start `rtk cargo run`.
- [ ] Verify UI health at `http://127.0.0.1:8520`.
- [ ] Verify token creation and authorized Demo Item list query.
- [ ] Verify `/apiConfig/parseDynamicSql`.
- [ ] Verify `/apiConfig/sql/execute`.
- [ ] Verify access monitor endpoints show API calls.

