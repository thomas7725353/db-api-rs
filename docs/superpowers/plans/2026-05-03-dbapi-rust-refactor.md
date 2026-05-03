# db-api Rust Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor Java db-api to a high-performance Rust implementation (Runtime Engine) while maintaining compatibility with existing metadata.

**Architecture:** A high-performance REST engine using `axum`, `sqlparser-rs` for safe parameter binding, `rbatis` for multi-instance data source management, and `moka` for configuration caching.

**Tech Stack:** Rust, axum, rbatis, rbdc (mysql, pg, sqlite), sqlparser, moka, tokio, serde.

---

### Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Initialize Cargo project**
Run: `cargo init`

- [ ] **Step 2: Add dependencies to Cargo.toml**
```toml
[package]
name = "db-api-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.7"
tower-http = { version = "0.5", features = ["cors"] }
rbatis = { version = "4.0" }
rbdc-sqlite = "4.0"
rbdc-mysql = "4.0"
rbdc-pg = "4.0"
sqlparser = "0.43"
moka = { version = "0.12", features = ["future"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dashmap = "5.5"
anyhow = "1.0"
log = "0.4"
env_logger = "0.10"
```

- [ ] **Step 3: Basic main.rs with Axum setup**
```rust
use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let app = Router::new().route("/health", get(|| async { "OK" }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    println!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 4: Commit**
```bash
git add Cargo.toml src/main.rs
git commit -m "chore: initial project scaffolding with axum"
```

---

### Task 2: Metadata Compatibility & Repository

**Files:**
- Create: `src/model.rs`
- Create: `src/repository.rs`

- [ ] **Step 1: Define models (compatible with data.db)**
Ensure table names and field names match the existing SQLite schema exactly.

- [ ] **Step 2: Implement ConfigRepository**
Logic for loading `ApiConfig` and `DataSource` from the existing `data.db`.

---

### Task 3: SQL Transformation & Binding Engine (Security)

**Files:**
- Create: `src/sql_engine.rs`

- [ ] **Step 1: Implement SqlTransformer**
Parse SQL, identify `$param`, replace with dialect placeholders (`?` or `$1`), and return the SQL + parameter order.

- [ ] **Step 2: Implement Security Guards**
Reject non-SELECT queries and multi-statement queries.

---

### Task 4: Multi-Instance RBatis Manager

**Files:**
- Create: `src/pool_manager.rs`

- [ ] **Step 1: Implement PoolManager**
A `DashMap<i32, RBatis>` to manage one `RBatis` instance per data source.

- [ ] **Step 2: Dynamic Initialization**
Initialize the correct driver based on the `db_type` from the metadata.

---

### Task 5: Axum Handler & Pipeline Integration

**Files:**
- Create: `src/handler.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement the dynamic `/api/:path` handler**
Pipeline: Axum Request -> Moka Cache -> SQL Transformer -> Parameter Binding -> RBatis Instance Exec -> JSON Response.

- [ ] **Step 2: Assembly in main.rs**
Connect the handler to the Axum router.
