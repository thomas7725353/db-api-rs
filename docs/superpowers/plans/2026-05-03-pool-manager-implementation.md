# Multi-Instance RBatis Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `PoolManager` in Rust to handle multiple `RBatis` instances for different data sources, replacing the Java `PoolManager`.

**Architecture:** Use `DashMap<i32, RBatis>` for thread-safe caching of `RBatis` instances. Each `RBatis` instance is initialized with the appropriate `rbdc` driver based on the `DataSource` type and URL.

**Tech Stack:** Rust, `rbatis`, `dashmap`, `rbdc-sqlite`, `rbdc-mysql`, `rbdc-pg`.

---

### Task 1: Module Setup and Struct Definition

**Files:**
- Create: `src/pool_manager.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/pool_manager.rs` with basic struct**

```rust
use dashmap::DashMap;
use rbatis::RBatis;
use crate::model::DataSource;

pub struct PoolManager {
    pools: DashMap<i32, RBatis>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }
}
```

- [ ] **Step 2: Register module in `src/main.rs`**

```rust
pub mod pool_manager;
```

- [ ] **Step 3: Commit**

```bash
git add src/pool_manager.rs src/main.rs
git commit -m "feat: initial PoolManager setup"
```

### Task 2: Implement URL conversion and Driver selection

**Files:**
- Modify: `src/pool_manager.rs`

- [ ] **Step 1: Write failing test for SQLite instance creation**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataSource;

    #[tokio::test]
    async fn test_get_or_create_sqlite() {
        let manager = PoolManager::new();
        let ds = DataSource {
            id: Some(1),
            name: Some("test_sqlite".to_string()),
            note: None,
            url: Some("jdbc:sqlite:test.db".to_string()),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        
        let rb = manager.get_or_create(&ds).await.unwrap();
        assert!(rb.is_init());
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run: `cargo test pool_manager::tests::test_get_or_create_sqlite`
Expected: FAIL (method not implemented)

- [ ] **Step 3: Implement `get_or_create` and helper methods**

```rust
use rbdc_sqlite::driver::SqliteDriver;
use rbdc_mysql::driver::MysqlDriver;
use rbdc_pg::driver::PgDriver;
use anyhow::Result;

impl PoolManager {
    // ...
    pub async fn get_or_create(&self, ds: &DataSource) -> Result<RBatis> {
        let id = ds.id.ok_or_else(|| anyhow::anyhow!("DataSource ID is missing"))?;
        
        if let Some(rb) = self.pools.get(&id) {
            return Ok(rb.clone());
        }

        let rb = self.create_rbatis(ds).await?;
        self.pools.insert(id, rb.clone());
        Ok(rb)
    }

    async fn create_rbatis(&self, ds: &DataSource) -> Result<RBatis> {
        let rb = RBatis::new();
        let url = ds.url.as_deref().unwrap_or("");
        let db_type = ds.db_type.as_deref().unwrap_or("");

        match db_type {
            "mysql" => {
                let url = url.replace("jdbc:mysql://", "mysql://");
                rb.init(MysqlDriver {}, &url)?;
            }
            "postgres" | "postgreSql" => {
                let url = url.replace("jdbc:postgresql://", "postgres://");
                rb.init(PgDriver {}, &url)?;
            }
            "sqlite" => {
                let url = url.replace("jdbc:sqlite:", "sqlite://");
                rb.init(SqliteDriver {}, &url)?;
            }
            _ => return Err(anyhow::anyhow!("Unsupported database type: {}", db_type)),
        }
        Ok(rb)
    }
}
```

- [ ] **Step 4: Run test and verify pass**

Run: `cargo test pool_manager::tests::test_get_or_create_sqlite`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pool_manager.rs
git commit -m "feat: implement get_or_create in PoolManager"
```

### Task 3: Implement Instance Removal and Refresh

**Files:**
- Modify: `src/pool_manager.rs`

- [ ] **Step 1: Write test for removing instance**

```rust
    #[tokio::test]
    async fn test_remove_pool() {
        let manager = PoolManager::new();
        let ds = DataSource {
            id: Some(1),
            name: Some("test_sqlite".to_string()),
            note: None,
            url: Some("jdbc:sqlite:test_remove.db".to_string()),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        
        manager.get_or_create(&ds).await.unwrap();
        assert!(manager.pools.contains_key(&1));
        
        manager.remove(1);
        assert!(!manager.pools.contains_key(&1));
    }
```

- [ ] **Step 2: Run test and verify failure**

- [ ] **Step 3: Implement `remove`**

```rust
impl PoolManager {
    // ...
    pub fn remove(&self, id: i32) {
        self.pools.remove(&id);
    }
}
```

- [ ] **Step 4: Run test and verify pass**

- [ ] **Step 5: Commit**

```bash
git add src/pool_manager.rs
git commit -m "feat: add remove method to PoolManager"
```

### Task 4: Final Validation and Integration

- [ ] **Step 1: Run all tests in the project**
Run: `cargo test`

- [ ] **Step 2: Final commit and cleanup**
