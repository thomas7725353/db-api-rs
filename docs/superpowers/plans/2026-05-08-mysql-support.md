# MySQL Datasource Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Productize MySQL as a first-class business datasource with real connection checks, local compose demo data, metadata seed APIs, and end-to-end verification.

**Architecture:** Keep SQLite as metadata storage and reuse the existing SeaORM runtime path. Add a narrow datasource connection-test helper in `src/db.rs`, wire `/datasource/connect` to it, and add MySQL compose/init/seed assets that mirror the PostgreSQL demo.

**Tech Stack:** Rust 2024, Axum, SeaORM/sqlx-mysql, SeaQuery, SQLite metadata, MySQL 8 Docker image, Ant Design frontend.

---

## File Structure

- Modify `src/db.rs`: expose real datasource connection testing and add focused MySQL normalization tests.
- Modify `src/datasource_handler.rs`: make `/datasource/connect` perform a real connection test.
- Modify `src/schema.rs`: expose MySQL column parsing to unit tests without needing a live MySQL connection.
- Modify `src/query_dsl.rs`: add MySQL QueryBuilder preview coverage.
- Modify `docker-compose.yml`: add MySQL service and dependency.
- Create `docker/mysql/init/001-demo-items.sql`: MySQL demo business table and deterministic rows.
- Create `seed_mysql_demo_api.sql`: deterministic SQLite metadata seed for `mysql_demo` and `/mysql/demo/items/*`.
- Verify existing frontend files compile without UI changes.

## Task 1: Real Datasource Connection Test

**Files:**
- Modify: `src/db.rs`
- Modify: `src/datasource_handler.rs`

- [ ] **Step 1: Add failing backend tests for connection helper shape**

Add tests in `src/db.rs` inside the existing `tests` module:

```rust
#[test]
fn normalizes_mysql_aliases_and_native_urls() {
    assert_eq!(
        normalize_url_with_base(
            "mysql",
            "mysql://dbapi:dbapi_pass@127.0.0.1:3306/dbapi_demo",
            None,
            None,
            None,
        )
        .unwrap(),
        "mysql://dbapi:dbapi_pass@127.0.0.1:3306/dbapi_demo"
    );
    assert_eq!(
        normalize_url_with_base(
            "mysql",
            "jdbc:mysql://127.0.0.1:3306/dbapi_demo",
            Some("dbapi"),
            Some("dbapi_pass"),
            None,
        )
        .unwrap(),
        "mysql://dbapi:dbapi_pass@127.0.0.1:3306/dbapi_demo"
    );
}

#[test]
fn rejects_unknown_datasource_type_for_normalization() {
    let error = normalize_url_with_base("oracle", "127.0.0.1/db", None, None, None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("Unsupported database type: oracle"));
}
```

- [ ] **Step 2: Run the focused tests**

Run:

```bash
rtk cargo test db::tests::normalizes_mysql_aliases_and_native_urls db::tests::rejects_unknown_datasource_type_for_normalization
```

Expected: tests compile and pass if existing normalization already covers them.

- [ ] **Step 3: Add a public connection-test helper**

In `src/db.rs`, add:

```rust
pub async fn test_data_source(ds: &DataSource, sqlite_base_dir: Option<&Path>) -> Result<()> {
    let db = connect_data_source_with_base(ds, sqlite_base_dir).await?;
    let sql = match db.backend {
        DbBackend::Sqlite => "select 1 as ok",
        DbBackend::MySql => "select 1 as ok",
        DbBackend::Postgres => "select 1 as ok",
    };
    db.conn.query_one(db.statement(sql, vec![])).await?;
    Ok(())
}
```

- [ ] **Step 4: Wire `/datasource/connect` to the helper**

Change `src/datasource_handler.rs`:

```rust
use crate::db;
```

Change the handler signature:

```rust
pub async fn connect(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
```

Replace the existing type-only success match with:

```rust
let ds = datasource_from_input(input);
match normalize_type(ds.db_type.as_deref()).as_str() {
    "mysql" | "postgres" | "sqlite" => {
        match db::test_data_source(&ds, state.pool_manager.sqlite_base_dir()).await {
            Ok(()) => dto_ok::<JsonValue>("连接成功", None).into_response(),
            Err(e) => dto_fail(format!("连接失败: {}", e)).into_response(),
        }
    }
    "hive" | "sqlserver" | "oracle" | "elasticsearch" => {
        dto_fail("Rust 单机版暂不支持该数据源类型").into_response()
    }
    other => dto_fail(format!("不支持的数据源类型: {}", other)).into_response(),
}
```

Expose the base dir from `DbPoolManager` in `src/db.rs`:

```rust
pub fn sqlite_base_dir(&self) -> Option<&Path> {
    self.sqlite_base_dir.as_deref()
}
```

- [ ] **Step 5: Run backend tests**

Run:

```bash
rtk cargo test db::tests datasource_handler
```

Expected: all selected tests pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/db.rs src/datasource_handler.rs
rtk git commit -m "feat: validate datasource connections"
```

## Task 2: MySQL Query And Schema Unit Coverage

**Files:**
- Modify: `src/query_dsl.rs`
- Modify: `src/schema.rs`
- Test: Rust unit tests in the same files

- [ ] **Step 1: Add MySQL QueryBuilder preview test**

In `src/query_dsl.rs` tests module, add a test that builds with `DbBackend::MySql`:

```rust
#[test]
fn mysql_preview_uses_mysql_quoting_and_placeholders() {
    let dsl = QueryBuilderDsl {
        r#type: "queryBuilder".to_string(),
        table: "demo_items".to_string(),
        select: vec!["id".to_string(), "name".to_string()],
        rules: RuleGroup {
            combinator: "and".to_string(),
            rules: vec![RuleNode::Rule(Rule {
                field: "status".to_string(),
                operator: "=".to_string(),
                value_source: Some("param".to_string()),
                value: json!({"param": "status"}),
                default_value: None,
                skip_when: vec![],
            })],
        },
        order_by: vec![OrderBy {
            field: "id".to_string(),
            direction: "desc".to_string(),
        }],
        limit: Some(PageSpec {
            param: None,
            default: Some(20),
            max: Some(100),
        }),
        offset: Some(PageSpec {
            param: None,
            default: Some(0),
            max: None,
        }),
        count: true,
    };

    let preview = parse_preview(&dsl, &json!({"status": "active"}), DbBackend::MySql).unwrap();

    assert!(preview.sql.contains("`demo_items`"));
    assert!(preview.sql.contains("WHERE `status` = ?"));
    assert!(preview.sql.contains("LIMIT ? OFFSET ?"));
    assert!(preview.count_sql.unwrap().contains("COUNT(*) AS `total`"));
}
```

- [ ] **Step 2: Make MySQL column parsing testable**

In `src/schema.rs`, keep `mysql_column` private but add this unit test in the tests module:

```rust
#[test]
fn mysql_column_maps_primary_key_nullable_default_and_auto_increment() {
    let column = mysql_column(serde_json::json!({
        "name": "id",
        "type": "bigint",
        "column_key": "PRI",
        "is_nullable": "NO",
        "column_default": null,
        "extra": "auto_increment"
    }))
    .unwrap();

    assert_eq!(column.name, "id");
    assert_eq!(column.column_type, "bigint");
    assert!(column.primary_key);
    assert_eq!(column.nullable, Some(false));
    assert!(column.generated);
}
```

- [ ] **Step 3: Run focused tests**

Run:

```bash
rtk cargo test mysql_preview_uses_mysql_quoting_and_placeholders mysql_column_maps_primary_key_nullable_default_and_auto_increment
```

Expected: both tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add src/query_dsl.rs src/schema.rs
rtk git commit -m "test: cover mysql query and schema paths"
```

## Task 3: MySQL Compose Service And Business Data

**Files:**
- Modify: `docker-compose.yml`
- Create: `docker/mysql/init/001-demo-items.sql`

- [ ] **Step 1: Add MySQL init SQL**

Create `docker/mysql/init/001-demo-items.sql`:

```sql
CREATE TABLE IF NOT EXISTS demo_items (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  status VARCHAR(64) NOT NULL DEFAULT 'active',
  note TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

INSERT INTO demo_items (name, status, note)
SELECT 'Alpha MySQL', 'active', 'mysql demo item'
WHERE NOT EXISTS (SELECT 1 FROM demo_items WHERE name = 'Alpha MySQL');
```

- [ ] **Step 2: Add MySQL service to compose**

Modify `docker-compose.yml` to add:

```yaml
  mysql:
    image: mysql:8
    environment:
      MYSQL_DATABASE: dbapi_demo
      MYSQL_USER: dbapi
      MYSQL_PASSWORD: dbapi_pass
      MYSQL_ROOT_PASSWORD: dbapi_root_pass
    ports:
      - "127.0.0.1:13306:3306"
    volumes:
      - mysql-data:/var/lib/mysql
      - ./docker/mysql/init:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "mysqladmin ping -h 127.0.0.1 -udbapi -pdbapi_pass --silent"]
      interval: 5s
      timeout: 3s
      retries: 30
```

Add `db-api-rs` dependency:

```yaml
      mysql:
        condition: service_healthy
```

Add volume:

```yaml
  mysql-data:
```

- [ ] **Step 3: Validate compose config**

Run:

```bash
rtk docker compose config
```

Expected: config renders successfully and includes `mysql`.

- [ ] **Step 4: Commit**

```bash
rtk git add docker-compose.yml docker/mysql/init/001-demo-items.sql
rtk git commit -m "feat: add mysql demo service"
```

## Task 4: MySQL Metadata Seed

**Files:**
- Create: `seed_mysql_demo_api.sql`

- [ ] **Step 1: Create deterministic metadata seed**

Create `seed_mysql_demo_api.sql`:

```sql
INSERT OR IGNORE INTO api_group (id, name) VALUES ('mysql_crud_group', 'mysql crud');

INSERT OR REPLACE INTO datasource (id, name, note, type, url, username, password, driver, table_sql, create_time, update_time)
VALUES (
  'mysql_demo',
  'MySQL 示例库',
  'Docker Compose MySQL business datasource',
  'mysql',
  'mysql://mysql:3306/dbapi_demo',
  'dbapi',
  'dbapi_pass',
  'com.mysql.cj.jdbc.Driver',
  NULL,
  datetime('now', 'localtime'),
  datetime('now', 'localtime')
);

DELETE FROM api_sql WHERE api_id IN (
  'mysql_demo_item_create', 'mysql_demo_item_get', 'mysql_demo_item_update', 'mysql_demo_item_delete',
  'mysql_demo_item_qb_list', 'mysql_demo_item_view_sql_list'
);
DELETE FROM api_config WHERE id IN (
  'mysql_demo_item_create', 'mysql_demo_item_get', 'mysql_demo_item_update', 'mysql_demo_item_delete',
  'mysql_demo_item_qb_list', 'mysql_demo_item_view_sql_list'
);

INSERT INTO api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param)
VALUES
('mysql_demo_item_create', '/mysql/demo/items/create', 'POST', 'MySQL 创建 Demo Item', 'POST name/status/note 创建 MySQL 记录', '[{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_get', '/mysql/demo/items/get', 'GET', 'MySQL 查询 Demo Item', '按 id 查询 MySQL 单条记录', '[{"name":"id","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_update', '/mysql/demo/items/update', 'PATCH', 'MySQL 更新 Demo Item', '按 id 更新 MySQL name/status/note', '[{"name":"id","type":"bigint"},{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_delete', '/mysql/demo/items/delete', 'DELETE', 'MySQL 删除 Demo Item', '按 id 删除 MySQL 记录', '[{"name":"id","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_qb_list', '/mysql/demo/items/qb-list', 'GET', 'MySQL Demo Item QueryBuilder List', 'MySQL QueryBuilder 列表接口', '[{"name":"keyword","type":"string"},{"name":"status","type":"string"},{"name":"limit","type":"bigint"},{"name":"offset","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_view_sql_list', '/mysql/demo/items/view-sql-list', 'GET', 'MySQL Demo Item View SQL List', 'MySQL View SQL 列表接口', '[{"name":"status","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL);

INSERT INTO api_sql (api_id, sql_text, transform_plugin, transform_plugin_params)
VALUES
('mysql_demo_item_create', 'INSERT INTO demo_items (name, status, note) VALUES ($name, $status, $note)', 'sql', ''),
('mysql_demo_item_get', 'SELECT id, name, status, note, created_at, updated_at FROM demo_items WHERE id = $id', 'sql', 'resultType=object'),
('mysql_demo_item_update', 'UPDATE demo_items SET name = $name, status = $status, note = $note, updated_at = current_timestamp WHERE id = $id', 'sql', ''),
('mysql_demo_item_delete', 'DELETE FROM demo_items WHERE id = $id', 'sql', ''),
('mysql_demo_item_qb_list', '{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true}', 'queryBuilder', 'resultType=page'),
('mysql_demo_item_view_sql_list', 'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]', 'viewSql', 'resultType=page'),
('mysql_demo_item_view_sql_list', 'select count(*) as total from demo_items a where a.status = $status', 'viewSqlCount', '');
```

- [ ] **Step 2: Validate SQL applies to a temporary copy**

Run:

```bash
rtk proxy cp data.db /tmp/dbapi-mysql-seed-test.db
rtk sqlite3 /tmp/dbapi-mysql-seed-test.db < seed_mysql_demo_api.sql
rtk sqlite3 /tmp/dbapi-mysql-seed-test.db "select id, type, url from datasource where id = 'mysql_demo';"
rtk sqlite3 /tmp/dbapi-mysql-seed-test.db "select count(*) from api_config where group_id = 'mysql_crud_group';"
```

Expected:

```text
mysql_demo|mysql|mysql://mysql:3306/dbapi_demo
6
```

- [ ] **Step 3: Commit**

```bash
rtk git add seed_mysql_demo_api.sql
rtk git commit -m "feat: seed mysql demo apis"
```

## Task 5: Full Verification And Direct Push

**Files:**
- No planned source edits unless verification exposes a bug.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
rtk cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run frontend tests and build**

Run:

```bash
rtk npm --prefix frontend test -- --run
rtk npm --prefix frontend run build
```

Expected: Vitest passes and Vite build completes.

- [ ] **Step 3: Start compose stack**

Run:

```bash
rtk docker compose up -d --build
rtk docker compose ps
```

Expected: `postgres`, `mysql`, `db-api-rs`, and `dbapi-mcp` are running; MySQL and PostgreSQL are healthy.

- [ ] **Step 4: Apply seed to local metadata**

Run:

```bash
rtk sqlite3 data.db < seed_mysql_demo_api.sql
```

Expected: command exits successfully.

- [ ] **Step 5: Verify health and MySQL APIs**

Run:

```bash
rtk curl -fsS http://127.0.0.1:8520/health
rtk curl -fsS "http://127.0.0.1:8520/api/mysql/demo/items/qb-list?status=active&limit=10&offset=0"
rtk curl -fsS "http://127.0.0.1:8520/api/mysql/demo/items/view-sql-list?status=active&columns=id,name,status,note,created_at,updated_at&order_by=id&limit=10&offset=0"
rtk curl -fsS -X POST http://127.0.0.1:8520/api/mysql/demo/items/create -d "name=Beta%20MySQL&status=active&note=created%20from%20smoke"
rtk curl -fsS "http://127.0.0.1:8520/api/mysql/demo/items/get?id=1"
rtk curl -fsS -X PATCH http://127.0.0.1:8520/api/mysql/demo/items/update -d "id=1&name=Alpha%20MySQL%20Updated&status=active&note=updated%20from%20smoke"
rtk curl -fsS -X DELETE http://127.0.0.1:8520/api/mysql/demo/items/delete -d "id=1"
```

Expected: every response has `"success":true`.

- [ ] **Step 6: Verify PostgreSQL demo still works**

Run:

```bash
rtk curl -fsS "http://127.0.0.1:8520/api/pg/demo/items/qb-list?status=active&limit=10&offset=0"
```

Expected: response has `"success":true`.

- [ ] **Step 7: Review git history and push main**

Run:

```bash
rtk git status --short
rtk git log --oneline -5
rtk git push origin main
```

Expected: worktree is clean before push; push succeeds to `origin/main`.
