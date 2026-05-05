# runtime-rust 3.3.0 Standalone Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Make `runtime-rust` run the verified db-api 3.3.0 frontend and backend-compatible single-machine service without Nacos.

**Architecture:** Rust serves the copied 3.3.0 Vue static assets and exposes Java-compatible management routes on one Axum 0.8 server. Metadata access uses the real 3.3.0 SQLite schema, including text IDs and `api_sql`; dynamic APIs load online metadata and execute bound SQL through per-datasource RBatis pools.

**Tech Stack:** Rust 2024, Axum 0.8, Tower HTTP static file serving, RBatis/RBDC 4.9, Tokio, Serde, SQLParser, Tracing, SQLite metadata.

---

### Task 1: Static Frontend Packaging

**Files:**
- Create: `runtime-rust/static/**`
- Modify: `runtime-rust/Cargo.toml`
- Modify: `runtime-rust/src/main.rs`

- [x] Copy `/Users/andy/IdeaProjects/db-api/dbapi-ui/dist` into `runtime-rust/static`.
- [x] Enable `tower-http` `fs` feature.
- [x] Serve static files with SPA fallback to `index.html`.
- [x] Verify `GET /` returns the 3.3.0 `index.html`.

### Task 2: 3.3.0 Data Models

**Files:**
- Modify: `runtime-rust/src/model.rs`
- Modify: `runtime-rust/src/response.rs`

- [x] Replace simplified integer-ID models with Java-compatible text-ID models.
- [x] Add `ApiSql`, `ApiGroup`, and `User` structs. Do not add firewall, token, app authorization, or API auth domain models.
- [x] Use serde renames for Java field names.
- [x] Change `ResponseDto` from `message` to Java `msg`.
- [x] Add serialization tests for `datasourceId`, `sqlList`, and `msg`.

### Task 3: 3.3.0 Repository Helpers

**Files:**
- Modify: `runtime-rust/src/repository.rs`
- Modify: `runtime-rust/src/model.rs`

- [x] Add in-memory 3.3.0 schema setup for tests.
- [x] Add CRUD helpers for datasource, api_config, api_sql, api_group, and users.
- [x] Add API detail loader that joins `api_config`, `api_sql`, and `api_alarm`.
- [x] Add online path loader for dynamic `/api/{path}`.
- [x] Add tests proving API details include `sqlList`.

### Task 4: Java-Compatible Management Handlers

**Files:**
- Modify: `runtime-rust/src/api_config_handler.rs`
- Modify: `runtime-rust/src/datasource_handler.rs`
- Create: `runtime-rust/src/basic_handler.rs`
- Modify: `runtime-rust/src/main.rs`

- [x] Implement system, user, group, plugin, and table routes.
- [x] Add inert compatibility responses for firewall and app/token authorization URLs so the old frontend does not receive 404s, but do not persist or enforce those features.
- [x] Rework datasource CRUD for text IDs and 3.3.0 fields.
- [x] Rework API CRUD to write `api_config` and `api_sql` transactionally.
- [x] Return Java-compatible raw list/object/null or `ResponseDto` per route.
- [x] Add handler tests for login, `/system/mode`, `/apiConfig/getAll`, `/group/getAll`, and `/plugin/all`.

### Task 5: Dynamic API Execution

**Files:**
- Modify: `runtime-rust/src/handler.rs`
- Modify: `runtime-rust/src/pool_manager.rs`
- Modify: `runtime-rust/src/sql_engine.rs`

- [x] Load first SQL statement from `ApiConfig.sqlList`.
- [x] Support text datasource IDs and normalize datasource type names.
- [x] Keep parameter binding and reject invalid parameter metadata.
- [x] Return Java-compatible `success/msg/data` dynamic API responses.
- [x] Add integration test using a file-backed SQLite datasource.

### Task 6: Verification And Runtime Smoke Test

**Files:**
- Modify as needed based on test failures.

- [x] Run `cargo test` in `runtime-rust`.
- [x] Run `cargo check` in `runtime-rust`.
- [x] Start Rust on `127.0.0.1:8520` after stopping Java if needed.
- [x] Verify `/`, `/user/login`, `/system/mode`, `/apiConfig/getAll`, `/datasource/getAll`, `/group/getAll`, and `/plugin/all` with curl.
