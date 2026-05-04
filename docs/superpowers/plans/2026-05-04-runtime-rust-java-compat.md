# runtime-rust Java Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `runtime-rust` a single-machine drop-in replacement for the Java backend used by the existing Vue frontend.

**Architecture:** Keep the existing SQLite metadata tables and current Vue request behavior. Split Rust into focused modules for Java-compatible models, form extraction, response DTOs, management APIs, dynamic API execution, pool management, and Docker packaging.

**Tech Stack:** Rust 2024, Axum 0.8, RBatis/RBDC 4.9, Tokio, Moka, Serde, SQLParser, Tracing, Docker Compose, Vue 2 static frontend served by Nginx.

---

### Task 1: Dependency And Runtime Baseline

**Files:**
- Modify: `runtime-rust/Cargo.toml`
- Modify: `runtime-rust/src/main.rs`
- Modify: `runtime-rust/Cargo.lock`

- [ ] Update `Cargo.toml` to Rust 2024, Axum 0.8, tower-http 0.6, RBatis/RBDC 4.9, and tracing.
- [ ] Replace `env_logger::init()` and `println!` with `tracing_subscriber` and `tracing::info!`.
- [ ] Run `cargo check` in `runtime-rust` and fix API breakage from dependency upgrades.
- [ ] Commit with `chore: upgrade rust runtime dependencies`.

### Task 2: Java-Compatible Models And Repository Methods

**Files:**
- Modify: `runtime-rust/src/model.rs`
- Modify: `runtime-rust/src/repository.rs`
- Create: `runtime-rust/src/response.rs`

- [ ] Add tests proving `ApiConfig` serializes as `datasourceId` and string `params`.
- [ ] Keep DB mapping to `datasource_id` while exposing frontend JSON as `datasourceId`.
- [ ] Add repository methods for insert, update, delete, duplicate path checks, online/offline, datasource reference counting, and datasource CRUD.
- [ ] Add Java-compatible `ResponseDto`.
- [ ] Run model/repository tests.
- [ ] Commit with `feat: add java compatible metadata models`.

### Task 3: Request Extraction And Management APIs

**Files:**
- Create: `runtime-rust/src/form.rs`
- Create: `runtime-rust/src/api_config_handler.rs`
- Create: `runtime-rust/src/datasource_handler.rs`
- Modify: `runtime-rust/src/main.rs`
- Modify: `runtime-rust/src/handler.rs`

- [ ] Add failing tests for form-urlencoded extraction.
- [ ] Implement shared extraction that accepts query, form-urlencoded body, and JSON object body.
- [ ] Implement `/apiConfig/*` routes used by Vue.
- [ ] Implement `/datasource/*` routes used by Vue.
- [ ] Invalidate config cache after API config mutations.
- [ ] Invalidate datasource pools after datasource update/delete.
- [ ] Add route registration in `main.rs`.
- [ ] Run handler tests.
- [ ] Commit with `feat: implement java management api compatibility`.

### Task 4: Dynamic API Compatibility Hardening

**Files:**
- Modify: `runtime-rust/src/handler.rs`
- Modify: `runtime-rust/src/sql_engine.rs`
- Modify: `runtime-rust/src/pool_manager.rs`

- [ ] Add failing test proving `/api/{path}` accepts form-urlencoded body parameters.
- [ ] Reject unknown parameter metadata types instead of silently accepting them.
- [ ] Keep bind-parameter SQL execution and reject leftover `$param` placeholders after transformation.
- [ ] Normalize datasource type names including `postgreSql`.
- [ ] Return concise client-safe errors instead of raw driver errors.
- [ ] Run dynamic API tests.
- [ ] Commit with `fix: harden dynamic api compatibility`.

### Task 5: Docker Compose Packaging

**Files:**
- Create: `runtime-rust/Dockerfile`
- Create: `src/main/webapp/Dockerfile`
- Create: `src/main/webapp/nginx.conf`
- Create: `docker-compose.yml`
- Create or modify: `.dockerignore`

- [ ] Add a multi-stage Rust backend image exposing port 8520.
- [ ] Add a Vue build image served by Nginx on port 8521.
- [ ] Configure Nginx to proxy `/api/`, `/apiConfig/`, `/datasource/`, and `/health` to `db-api-rs:8520`.
- [ ] Add root `docker-compose.yml` with backend and frontend services plus persistent `data.db` bind mount.
- [ ] Run `docker compose config`.
- [ ] If Docker is available, run `docker compose up --build` smoke verification.
- [ ] Commit with `feat: add single machine compose packaging`.

### Task 6: Final Verification

**Files:**
- Modify as needed from previous tasks only.

- [ ] Run `cargo fmt`.
- [ ] Run `cargo test`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo check`.
- [ ] Run `docker compose config`.
- [ ] Report any Docker runtime verification skipped because Docker is unavailable.
- [ ] Commit final cleanup only if files changed.
