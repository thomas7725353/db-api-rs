# DBAPI Skills and MCP Sidecar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement DBAPI Manifest v1, repo-local API generation skills, CLI dry-run/apply flow, and a Docker Compose MCP HTTP sidecar that exposes the same API bundle workflow to agents.

**Architecture:** Keep the existing DBAPI HTTP server as the source of truth for management writes. Add a shared manifest/generator/validator layer in Rust, expose it through CLI commands and an rmcp-based HTTP sidecar, and refresh repo-local skills so agents generate reviewable files before applying them through existing import/group/app/token routes.

**Tech Stack:** Rust 2024, Axum, SeaORM/SQLx, SeaQuery QueryBuilder, SQLParser, MiniJinja View SQL, reqwest multipart client, clap CLI, rmcp MCP server, Docker Compose, repo-local Codex skills, Cargo tests.

---

## File Structure

- Modify `Cargo.toml`: add `clap`, `rmcp`, `schemars`, `tokio-util`, and reqwest multipart support.
- Modify `src/main.rs`: shrink to module declarations plus `cli::run().await`.
- Create `src/app.rs`: HTTP server bootstrap, Axum router assembly, static fallback, and existing HTML cache test.
- Create `src/cli.rs`: parse `serve`, `bundle`, and `mcp` commands.
- Create `src/schema.rs`: datasource table/column introspection with primary-key, nullable, default, and generated metadata.
- Modify `src/basic_handler.rs`: delegate `/table/getAllTables` and `/table/getAllColumns` to `schema.rs`.
- Create `src/manifest.rs`: DBAPI Manifest v1 structs, generated file structs, validation report structs, and shared input types.
- Create `src/manifest_generator.rs`: table CRUD/list/table/view generator and SQL API generator.
- Create `src/manifest_validator.rs`: dry-run validation against DBAPI metadata and datasource schema.
- Create `src/bundle_files.rs`: read/write generated `dbapi_manifest.json`, `api_group_config.json`, `api_config.json`, `curl.md`, and `VERIFY.md`.
- Create `src/dbapi_client.rs`: HTTP client for datasource/schema/export/import/app/token routes.
- Create `src/mcp_server.rs`: rmcp HTTP sidecar service and tools.
- Create `skills/dbapi-generate-table-apis/SKILL.md`
- Create `skills/dbapi-generate-sql-api/SKILL.md`
- Create `skills/dbapi-apply-api-bundle/SKILL.md`
- Create `skills/dbapi-token-workflow/SKILL.md`
- Create `skills/dbapi-export-import-workflow/SKILL.md`
- Modify `docker-compose.yml`: add `dbapi-mcp` service.
- Modify `Dockerfile`: expose `8521`.
- Modify `README.md`: document manifest/CLI/MCP sidecar usage, repo-local skills, human workflows, agent workflows, and write-safety rules so both users and agents can understand the feature from the repository entrypoint.

## Task 1: Split HTTP Server From Binary Entry

**Files:**
- Modify: `src/main.rs`
- Create: `src/app.rs`

- [ ] **Step 1: Move server setup into `src/app.rs`**

Create `src/app.rs` with this structure, moving the existing router, static fallback, and `index_response` test out of `src/main.rs`:

```rust
use crate::db::DbPoolManager;
use crate::{
    api_config_handler, basic_handler, datasource_handler, db, handler, query_builder_handler,
};
use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing::info;

pub async fn serve_http() -> anyhow::Result<()> {
    let metadata_url =
        std::env::var("DB_API_METADATA_URL").unwrap_or_else(|_| "sqlite://../data.db".to_string());
    let metadata_db = crate::repository::init_repository(&metadata_url).await?;
    let pool_manager = Arc::new(DbPoolManager::new(db::sqlite_base_dir_from_url(
        &metadata_url,
    )));
    let state = Arc::new(handler::AppState::new(metadata_db, pool_manager));
    let app = router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    info!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router(state: Arc<handler::AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/{*path}", any(handler::handle_api))
        .route("/user/login", post(basic_handler::login))
        .route("/user/resetPassword", post(basic_handler::reset_password))
        .route("/token/generate", any(basic_handler::token_generate))
        .route("/system/version", any(basic_handler::version))
        .route("/system/mode", any(basic_handler::mode))
        .route("/system/getIPPort", any(basic_handler::get_ip_port))
        .route("/system/getIP", any(basic_handler::get_ip))
        .route("/apiConfig/add", post(api_config_handler::add))
        .route("/apiConfig/update", post(api_config_handler::update))
        .route("/apiConfig/getAll", any(api_config_handler::get_all))
        .route("/apiConfig/search", any(api_config_handler::search))
        .route("/apiConfig/getApiTree", any(api_config_handler::get_api_tree))
        .route("/apiConfig/downloadConfig", post(api_config_handler::download_config))
        .route("/apiConfig/import", post(api_config_handler::import_config))
        .route("/apiConfig/downloadGroupConfig", post(api_config_handler::download_group_config))
        .route("/apiConfig/importGroup", post(api_config_handler::import_group))
        .route("/apiConfig/apiDocs", post(api_config_handler::api_docs))
        .route("/apiConfig/context", any(api_config_handler::context))
        .route("/apiConfig/detail/{id}", any(api_config_handler::detail))
        .route("/apiConfig/delete/{id}", any(api_config_handler::delete))
        .route("/apiConfig/online/{id}", any(api_config_handler::online))
        .route("/apiConfig/offline/{id}", any(api_config_handler::offline))
        .route("/apiConfig/parseParam", post(api_config_handler::parse_param))
        .route("/apiConfig/parseDynamicSql", post(api_config_handler::parse_dynamic_sql))
        .route("/apiConfig/sql/execute", post(api_config_handler::execute_sql))
        .route("/queryBuilder/parse", post(query_builder_handler::parse))
        .route("/queryBuilder/execute", post(query_builder_handler::execute))
        .route("/datasource/add", post(datasource_handler::add))
        .route("/datasource/update", post(datasource_handler::update))
        .route("/datasource/getAll", any(datasource_handler::get_all))
        .route("/datasource/detail/{id}", any(datasource_handler::detail))
        .route("/datasource/delete/{id}", any(datasource_handler::delete))
        .route("/datasource/connect", post(datasource_handler::connect))
        .route("/group/getAll/", any(basic_handler::group_get_all))
        .route("/group/getAll", any(basic_handler::group_get_all))
        .route("/group/create/", post(basic_handler::group_create))
        .route("/group/create", post(basic_handler::group_create))
        .route("/group/delete/{id}", any(basic_handler::group_delete))
        .route("/plugin/all", any(basic_handler::plugin_all))
        .route("/firewall/detail", any(basic_handler::firewall_detail))
        .route("/firewall/save", post(basic_handler::firewall_save))
        .route("/app/create", post(basic_handler::app_create))
        .route("/app/getAll", post(basic_handler::app_get_all))
        .route("/app/delete/{id}", post(basic_handler::app_delete))
        .route("/app/auth/", post(basic_handler::app_auth))
        .route("/app/auth", post(basic_handler::app_auth))
        .route("/app/getAuthGroups/{id}", post(basic_handler::app_get_auth_groups))
        .route("/access/countByDay", post(basic_handler::access_count_by_day))
        .route("/access/successRatio", post(basic_handler::access_success_ratio))
        .route("/access/top5api", post(basic_handler::access_top5api))
        .route("/access/top5app", post(basic_handler::access_top5app))
        .route("/access/topNIP", post(basic_handler::access_top_n_ip))
        .route("/access/top5duration", post(basic_handler::access_top5duration))
        .route("/access/search", post(basic_handler::access_search))
        .route("/table/getAllTables", post(basic_handler::table_get_all_tables))
        .route("/table/getAllColumns", post(basic_handler::table_get_all_columns))
        .nest_service("/assets", ServeDir::new("static/assets"))
        .fallback(spa_index)
        .with_state(state)
}

async fn spa_index() -> Response {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(html) => index_response(html),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read static/index.html: {error}"),
        )
            .into_response(),
    }
}

fn index_response(html: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
            (header::PRAGMA, "no-cache"),
            (header::EXPIRES, "0"),
        ],
        html,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn index_response_disables_html_cache() {
        let response = index_response("<!doctype html>".to_string());

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store, no-cache, must-revalidate"
        );
        assert_eq!(response.headers().get(header::PRAGMA).unwrap(), "no-cache");
    }
}
```

- [ ] **Step 2: Replace `src/main.rs` with module declarations and `cli::run()`**

Replace `src/main.rs` with:

```rust
mod api_config_handler;
mod app;
mod basic_handler;
mod bundle_files;
mod cli;
mod datasource_handler;
mod db;
mod dbapi_client;
mod form;
mod handler;
mod manifest;
mod manifest_generator;
mod manifest_validator;
mod mcp_server;
mod model;
mod query_builder_handler;
mod query_dsl;
mod repository;
mod response;
mod schema;
mod sql_engine;
mod view_sql;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    cli::run().await
}
```

- [ ] **Step 3: Add temporary empty modules so the crate compiles**

Create these files with the minimal module bodies needed for compilation:

```rust
// src/bundle_files.rs
```

```rust
// src/cli.rs
pub async fn run() -> anyhow::Result<()> {
    crate::app::serve_http().await
}
```

```rust
// src/dbapi_client.rs
```

```rust
// src/manifest.rs
```

```rust
// src/manifest_generator.rs
```

```rust
// src/manifest_validator.rs
```

```rust
// src/mcp_server.rs
```

```rust
// src/schema.rs
```

- [ ] **Step 4: Run tests**

Run:

```bash
rtk cargo test app::tests::index_response_disables_html_cache
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/main.rs src/app.rs src/cli.rs src/bundle_files.rs src/dbapi_client.rs src/manifest.rs src/manifest_generator.rs src/manifest_validator.rs src/mcp_server.rs src/schema.rs
rtk git commit -m "refactor: split server bootstrap from binary entry"
```

## Task 2: Add CLI and MCP Dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/cli.rs`

- [ ] **Step 1: Update dependencies**

In `Cargo.toml`, change reqwest and add CLI/MCP dependencies:

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "multipart"] }
clap = { version = "4.5", features = ["derive"] }
rmcp = { version = "1.6", features = ["server", "macros", "schemars", "transport-streamable-http-server"] }
schemars = { version = "1.0", features = ["derive"] }
tokio-util = "0.7"
```

- [ ] **Step 2: Implement command parsing**

Replace `src/cli.rs` with:

```rust
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "db-api-rs")]
#[command(about = "DBAPI runtime, bundle generator, and MCP sidecar")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
    Bundle(BundleArgs),
    Mcp(McpArgs),
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "http")]
    pub transport: String,
    #[arg(long, default_value = "0.0.0.0:8521")]
    pub listen: String,
    #[arg(long, default_value = "http://127.0.0.1:8520")]
    pub base_url: String,
    #[arg(long, default_value_t = false)]
    pub allow_write: bool,
}

#[derive(Debug, Args)]
pub struct BundleArgs {
    #[command(subcommand)]
    pub command: BundleCommand,
}

#[derive(Debug, Subcommand)]
pub enum BundleCommand {
    DraftTable(DraftTableArgs),
    DraftSql(DraftSqlArgs),
    Validate(BundleIoArgs),
    Apply(BundleApplyArgs),
}

#[derive(Debug, Args)]
pub struct DraftTableArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub datasource_id: String,
    #[arg(long)]
    pub table: String,
    #[arg(long)]
    pub primary_key: Option<String>,
    #[arg(long)]
    pub resource_path: String,
    #[arg(long)]
    pub group_id: String,
    #[arg(long)]
    pub group_name: String,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct DraftSqlArgs {
    #[arg(long)]
    pub datasource_id: String,
    #[arg(long)]
    pub resource_path: String,
    #[arg(long)]
    pub api_id: String,
    #[arg(long)]
    pub api_name: String,
    #[arg(long)]
    pub group_id: String,
    #[arg(long)]
    pub group_name: String,
    #[arg(long)]
    pub sql: String,
    #[arg(long, default_value = "sql")]
    pub engine: String,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct BundleIoArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct BundleApplyArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub dir: PathBuf,
    #[arg(long, default_value_t = false)]
    pub allow_write: bool,
}

pub async fn run() -> anyhow::Result<()> {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => crate::app::serve_http().await,
        Command::Bundle(args) => crate::bundle_files::run_bundle_command(args).await,
        Command::Mcp(args) => crate::mcp_server::serve(args).await,
    }
}
```

- [ ] **Step 3: Add temporary stubs for new entry points**

In `src/bundle_files.rs`:

```rust
use crate::cli::{BundleArgs, BundleCommand};

pub async fn run_bundle_command(args: BundleArgs) -> anyhow::Result<()> {
    match args.command {
        BundleCommand::DraftTable(_) => anyhow::bail!("draft-table is not implemented yet"),
        BundleCommand::DraftSql(_) => anyhow::bail!("draft-sql is not implemented yet"),
        BundleCommand::Validate(_) => anyhow::bail!("validate is not implemented yet"),
        BundleCommand::Apply(_) => anyhow::bail!("apply is not implemented yet"),
    }
}
```

In `src/mcp_server.rs`:

```rust
use crate::cli::McpArgs;

pub async fn serve(_args: McpArgs) -> anyhow::Result<()> {
    anyhow::bail!("mcp server is not implemented yet")
}
```

- [ ] **Step 4: Verify CLI help compiles**

Run:

```bash
rtk cargo run -- --help
```

Expected: command exits successfully and prints `DBAPI runtime, bundle generator, and MCP sidecar`.

- [ ] **Step 5: Commit**

```bash
rtk git add Cargo.toml Cargo.lock src/cli.rs src/bundle_files.rs src/mcp_server.rs
rtk git commit -m "feat: add dbapi cli command surface"
```

## Task 3: Move and Extend Schema Introspection

**Files:**
- Create/modify: `src/schema.rs`
- Modify: `src/basic_handler.rs`

- [ ] **Step 1: Add failing schema tests**

Add this test module to `src/schema.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn sqlite_columns_include_primary_key_and_defaults() {
        let db = db::connect_metadata("sqlite::memory:").await.unwrap();
        db::execute(
            &db,
            "create table demo_items (id integer primary key autoincrement, name text not null, status text default 'active', note text)",
            vec![],
        )
        .await
        .unwrap();

        let schema = inspect_table(&db, "demo_items").await.unwrap();

        let id = schema.columns.iter().find(|column| column.name == "id").unwrap();
        assert!(id.primary_key);
        assert!(id.generated);
        let name = schema.columns.iter().find(|column| column.name == "name").unwrap();
        assert_eq!(name.nullable, Some(false));
        let status = schema.columns.iter().find(|column| column.name == "status").unwrap();
        assert_eq!(status.default_value.as_deref(), Some("'active'"));
    }
}
```

- [ ] **Step 2: Run the failing test**

```bash
rtk cargo test schema::tests::sqlite_columns_include_primary_key_and_defaults
```

Expected: FAIL because `inspect_table` and public schema structs are not implemented.

- [ ] **Step 3: Implement schema structs and SQLite inspection**

Replace `src/schema.rs` with:

```rust
use crate::db::{self, DbConn};
use anyhow::anyhow;
use sea_orm::DbBackend;
use sea_query::Value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub table: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
    pub primary_key: bool,
    pub nullable: Option<bool>,
    pub default_value: Option<String>,
    pub generated: bool,
}

pub async fn list_tables(data_db: &DbConn) -> anyhow::Result<Vec<String>> {
    let sql = match data_db.backend {
        DbBackend::Sqlite => {
            "select name from sqlite_master where type = 'table' and name not like 'sqlite_%' order by name"
        }
        DbBackend::MySql => {
            "select table_name as name from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' order by table_name"
        }
        DbBackend::Postgres => {
            "select table_name as name from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by table_name"
        }
    };
    let rows = db::query_json(data_db, sql, vec![]).await?;
    Ok(rows.into_iter().filter_map(extract_name).collect())
}

pub async fn inspect_table(data_db: &DbConn, table: &str) -> anyhow::Result<TableSchema> {
    validate_table_identifier(table)?;
    let columns = match data_db.backend {
        DbBackend::Sqlite => inspect_sqlite_table(data_db, table).await?,
        DbBackend::MySql => inspect_mysql_table(data_db, table).await?,
        DbBackend::Postgres => inspect_postgres_table(data_db, table).await?,
    };
    Ok(TableSchema {
        table: table.to_string(),
        columns,
    })
}

async fn inspect_sqlite_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let sql = format!("PRAGMA table_info(\"{}\")", escape_sqlite_identifier(table));
    let rows = db::query_json(data_db, &sql, vec![]).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let object = row.as_object()?;
            let name = object.get("name")?.as_str()?.to_string();
            let column_type = object
                .get("type")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string();
            let primary_key = int_field(object.get("pk")) > 0;
            let nullable = Some(int_field(object.get("notnull")) == 0 && !primary_key);
            let default_value = object
                .get("dflt_value")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            let generated = primary_key && column_type.to_ascii_lowercase().contains("int");
            Some(ColumnInfo {
                name,
                column_type,
                primary_key,
                nullable,
                default_value,
                generated,
            })
        })
        .collect())
}

async fn inspect_mysql_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let rows = db::query_json(
        data_db,
        "select column_name as name, data_type as type, column_key, is_nullable, column_default, extra from information_schema.columns where table_schema = database() and table_name = ? order by ordinal_position",
        vec![string_value(table)],
    )
    .await?;
    Ok(rows.into_iter().filter_map(mysql_column).collect())
}

async fn inspect_postgres_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let rows = db::query_json(
        data_db,
        "select c.column_name as name, c.data_type as type, case when tc.constraint_type = 'PRIMARY KEY' then 'PRI' else '' end as column_key, c.is_nullable, c.column_default, c.is_identity from information_schema.columns c left join information_schema.key_column_usage k on c.table_schema = k.table_schema and c.table_name = k.table_name and c.column_name = k.column_name left join information_schema.table_constraints tc on k.constraint_schema = tc.constraint_schema and k.constraint_name = tc.constraint_name where c.table_schema = 'public' and c.table_name = $1 order by c.ordinal_position",
        vec![string_value(table)],
    )
    .await?;
    Ok(rows.into_iter().filter_map(postgres_column).collect())
}

fn mysql_column(row: JsonValue) -> Option<ColumnInfo> {
    let object = row.as_object()?;
    let extra = object.get("extra").and_then(JsonValue::as_str).unwrap_or("");
    Some(ColumnInfo {
        name: object.get("name")?.as_str()?.to_string(),
        column_type: object.get("type")?.as_str()?.to_string(),
        primary_key: object.get("column_key").and_then(JsonValue::as_str) == Some("PRI"),
        nullable: object
            .get("is_nullable")
            .and_then(JsonValue::as_str)
            .map(|value| value.eq_ignore_ascii_case("YES")),
        default_value: object
            .get("column_default")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        generated: extra.to_ascii_lowercase().contains("auto_increment"),
    })
}

fn postgres_column(row: JsonValue) -> Option<ColumnInfo> {
    let object = row.as_object()?;
    let default_value = object
        .get("column_default")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let is_identity = object
        .get("is_identity")
        .and_then(JsonValue::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("YES"));
    Some(ColumnInfo {
        name: object.get("name")?.as_str()?.to_string(),
        column_type: object.get("type")?.as_str()?.to_string(),
        primary_key: object.get("column_key").and_then(JsonValue::as_str) == Some("PRI"),
        nullable: object
            .get("is_nullable")
            .and_then(JsonValue::as_str)
            .map(|value| value.eq_ignore_ascii_case("YES")),
        generated: is_identity
            || default_value
                .as_deref()
                .is_some_and(|value| value.contains("nextval(")),
        default_value,
    })
}

fn extract_name(row: JsonValue) -> Option<String> {
    match row {
        JsonValue::Object(object) => object
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .or_else(|| object.into_values().next()?.as_str().map(str::to_string)),
        JsonValue::String(value) => Some(value),
        _ => None,
    }
}

fn int_field(value: Option<&JsonValue>) -> i64 {
    value
        .and_then(JsonValue::as_i64)
        .or_else(|| value?.as_str()?.parse::<i64>().ok())
        .unwrap_or(0)
}

fn string_value(value: &str) -> Value {
    Value::String(Some(Box::new(value.to_string())))
}

pub fn validate_table_identifier(value: &str) -> anyhow::Result<()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!("table is required"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(anyhow!("Invalid table: {}", value));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(anyhow!("Invalid table: {}", value));
    }
    Ok(())
}

fn escape_sqlite_identifier(value: &str) -> String {
    value.replace('"', "\"\"")
}
```

- [ ] **Step 4: Make `basic_handler` use `schema.rs`**

In `src/basic_handler.rs`, remove the private `ColumnInfo`, `list_datasource_tables`, `list_datasource_columns`, `validate_table_identifier`, `extract_name`, `string_value`, and `escape_sqlite_identifier` helpers. Change the handlers to call:

```rust
match crate::schema::list_tables(&data_db).await {
    Ok(tables) => Json(json!(tables)).into_response(),
    Err(e) => dto_fail(format!("Failed to list tables: {}", e)).into_response(),
}
```

and:

```rust
match crate::schema::inspect_table(&data_db, &table).await {
    Ok(schema) => Json(json!(schema.columns)).into_response(),
    Err(e) => dto_fail(format!("Failed to list columns: {}", e)).into_response(),
}
```

- [ ] **Step 5: Run schema and existing handler tests**

```bash
rtk cargo test schema::tests::sqlite_columns_include_primary_key_and_defaults basic_handler::tests::rejects_unsafe_sqlite_table_identifier
```

Expected: PASS. If `basic_handler::tests::rejects_unsafe_sqlite_table_identifier` no longer exists after moving the helper, move that test into `schema.rs` and run `rtk cargo test schema::tests`.

- [ ] **Step 6: Commit**

```bash
rtk git add src/schema.rs src/basic_handler.rs
rtk git commit -m "feat: add schema introspection metadata"
```

## Task 4: Define DBAPI Manifest v1 Models

**Files:**
- Modify: `src/manifest.rs`

- [ ] **Step 1: Add serialization tests**

Create `src/manifest.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn draft_table_input_requires_resource_path_in_manifest_shape() {
        let input: DraftTableInput = serde_json::from_value(json!({
            "datasourceId": "postgres_demo",
            "table": "demo_items",
            "primaryKey": "id",
            "resourcePath": "demo/items",
            "group": {"id": "demo_items_group", "name": "Demo Items"}
        }))
        .unwrap();

        assert_eq!(input.resource_path, "demo/items");
        assert_eq!(input.group.id, "demo_items_group");
    }

    #[test]
    fn validation_report_is_success_when_no_errors() {
        let report = ValidationReport::default();
        assert!(report.success);
    }
}
```

- [ ] **Step 2: Implement manifest structs above the tests**

Add this code before the test module:

```rust
use crate::model::{ApiConfigExport, ApiGroup};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MANIFEST_VERSION: &str = "dbapi.manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftTableInput {
    pub datasource_id: String,
    pub table: String,
    pub primary_key: Option<String>,
    pub resource_path: String,
    pub group: ManifestGroup,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftSqlInput {
    pub datasource_id: String,
    pub resource_path: String,
    pub api_id: String,
    pub api_name: String,
    pub group: ManifestGroup,
    pub sql: String,
    #[serde(default = "default_sql_engine")]
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbapiManifest {
    pub version: String,
    pub source: ManifestSource,
    pub group_file: String,
    pub api_file: String,
    pub curl_file: String,
    pub verify_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSource {
    pub datasource_id: String,
    pub table: Option<String>,
    pub primary_key: Option<String>,
    pub resource_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedBundle {
    pub manifest: DbapiManifest,
    pub groups: Vec<ApiGroup>,
    pub api_config: ApiConfigExport,
    pub curl_md: String,
    pub verify_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn error(&mut self, message: impl Into<String>) {
        self.success = false;
        self.errors.push(message.into());
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
}

fn default_true() -> bool {
    true
}

fn default_sql_engine() -> String {
    "sql".to_string()
}
```

- [ ] **Step 3: Run manifest tests**

```bash
rtk cargo test manifest::tests
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
rtk git add src/manifest.rs Cargo.toml Cargo.lock
rtk git commit -m "feat: define dbapi manifest models"
```

## Task 5: Generate Table CRUD/List/Table/View Bundles

**Files:**
- Modify: `src/manifest_generator.rs`
- Modify: `src/manifest.rs`

- [ ] **Step 1: Add generator tests**

Create `src/manifest_generator.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DraftTableInput, ManifestGroup};
    use crate::schema::{ColumnInfo, TableSchema};

    #[test]
    fn table_bundle_generates_sql_querybuilder_and_view_sql_apis() {
        let schema = TableSchema {
            table: "demo_items".to_string(),
            columns: vec![
                col("id", "integer", true, true),
                col("name", "text", false, false),
                col("status", "text", false, false),
                col("note", "text", false, false),
                col("created_at", "timestamp", false, true),
                col("updated_at", "timestamp", false, true),
            ],
        };
        let bundle = draft_table_crud_bundle(
            DraftTableInput {
                datasource_id: "postgres_demo".to_string(),
                table: "demo_items".to_string(),
                primary_key: Some("id".to_string()),
                resource_path: "demo/items".to_string(),
                group: ManifestGroup {
                    id: "demo_items_group".to_string(),
                    name: "Demo Items".to_string(),
                },
                public: true,
            },
            &schema,
        )
        .unwrap();

        let paths = bundle
            .api_config
            .api
            .iter()
            .map(|api| api.path.as_deref().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "demo/items/create",
                "demo/items/get",
                "demo/items/update",
                "demo/items/delete",
                "demo/items/qb-list",
                "demo/items/table",
                "demo/items/view-sql-list",
            ]
        );

        assert_eq!(bundle.api_config.api.len(), 7);
        assert!(bundle.api_config.sql.iter().any(|row| row.transform_plugin.as_deref() == Some("queryBuilder")));
        assert!(bundle.api_config.sql.iter().any(|row| row.transform_plugin.as_deref() == Some("viewSql")));
        assert!(bundle.api_config.sql.iter().any(|row| row.transform_plugin.as_deref() == Some("viewSqlCount")));
        assert!(bundle.curl_md.contains("/api/demo/items/qb-list"));
        assert!(bundle.verify_md.contains("Validate generated API bundle"));
    }

    fn col(name: &str, ty: &str, primary_key: bool, generated: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            column_type: ty.to_string(),
            primary_key,
            nullable: Some(!primary_key),
            default_value: None,
            generated,
        }
    }
}
```

- [ ] **Step 2: Run the failing test**

```bash
rtk cargo test manifest_generator::tests::table_bundle_generates_sql_querybuilder_and_view_sql_apis
```

Expected: FAIL because `draft_table_crud_bundle` is not implemented.

- [ ] **Step 3: Implement generator helpers**

Add these imports and helper functions above the test module:

```rust
use crate::manifest::{
    DbapiManifest, DraftTableInput, GeneratedBundle, MANIFEST_VERSION, ManifestSource,
};
use crate::model::{ApiConfig, ApiConfigExport, ApiGroup, ApiSql};
use crate::schema::{ColumnInfo, TableSchema};
use anyhow::anyhow;
use serde_json::json;

pub fn draft_table_crud_bundle(
    input: DraftTableInput,
    schema: &TableSchema,
) -> anyhow::Result<GeneratedBundle> {
    let resource_path = normalize_resource_path(&input.resource_path)?;
    let primary_key = input
        .primary_key
        .clone()
        .or_else(|| schema.columns.iter().find(|column| column.primary_key).map(|column| column.name.clone()));
    let Some(primary_key) = primary_key else {
        return Err(anyhow!("primary_key is required when table metadata has no primary key"));
    };
    if !schema.columns.iter().any(|column| column.name == primary_key) {
        return Err(anyhow!("primary_key does not exist in table: {}", primary_key));
    }

    let writable_columns = schema
        .columns
        .iter()
        .filter(|column| column.name != primary_key && !column.generated)
        .cloned()
        .collect::<Vec<_>>();
    let selected_columns = schema.columns.iter().map(|column| column.name.clone()).collect::<Vec<_>>();
    let privilege = if input.public { 1 } else { 0 };
    let group_id = input.group.id.clone();
    let datasource_id = input.datasource_id.clone();

    let mut api = Vec::new();
    let mut sql = Vec::new();

    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{}_create", slug_id(&resource_path)),
            path: format!("{resource_path}/create"),
            method: "POST",
            name: format!("Create {}", schema.table),
            note: "Insert a row".to_string(),
            params: params_for_columns(&writable_columns),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: insert_sql(&schema.table, &writable_columns),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{}_get", slug_id(&resource_path)),
            path: format!("{resource_path}/get"),
            method: "GET",
            name: format!("Get {}", schema.table),
            note: "Read one row by primary key".to_string(),
            params: params_for_names(&[primary_key.as_str()], schema),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: format!("select {} from {} where {} = ${}", selected_columns.join(", "), schema.table, primary_key, primary_key),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{}_update", slug_id(&resource_path)),
            path: format!("{resource_path}/update"),
            method: "PATCH",
            name: format!("Update {}", schema.table),
            note: "Update one row by primary key".to_string(),
            params: [params_for_names(&[primary_key.as_str()], schema), params_for_columns(&writable_columns)].concat(),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: update_sql(&schema.table, &primary_key, &writable_columns),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{}_delete", slug_id(&resource_path)),
            path: format!("{resource_path}/delete"),
            method: "DELETE",
            name: format!("Delete {}", schema.table),
            note: "Delete one row by primary key".to_string(),
            params: params_for_names(&[primary_key.as_str()], schema),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: format!("delete from {} where {} = ${}", schema.table, primary_key, primary_key),
        },
    );
    push_query_builder_api(&mut api, &mut sql, &resource_path, "qb-list", schema, &datasource_id, &group_id, privilege);
    push_query_builder_api(&mut api, &mut sql, &resource_path, "table", schema, &datasource_id, &group_id, privilege);
    push_view_sql_api(&mut api, &mut sql, &resource_path, schema, &datasource_id, &group_id, privilege);

    Ok(GeneratedBundle {
        manifest: DbapiManifest {
            version: MANIFEST_VERSION.to_string(),
            source: ManifestSource {
                datasource_id,
                table: Some(schema.table.clone()),
                primary_key: Some(primary_key),
                resource_path,
            },
            group_file: "api_group_config.json".to_string(),
            api_file: "api_config.json".to_string(),
            curl_file: "curl.md".to_string(),
            verify_file: "VERIFY.md".to_string(),
        },
        groups: vec![ApiGroup {
            id: Some(input.group.id),
            name: Some(input.group.name),
        }],
        api_config: ApiConfigExport { api, sql },
        curl_md: generate_curl_md(),
        verify_md: generate_verify_md(),
    })
}
```

- [ ] **Step 4: Add the concrete SQL/API helper code**

Append these helpers above the test module:

```rust
struct SqlApiSpec<'a> {
    id: String,
    path: String,
    method: &'a str,
    name: String,
    note: String,
    params: Vec<serde_json::Value>,
    datasource_id: &'a str,
    group_id: &'a str,
    privilege: i32,
    sql_text: String,
}

fn push_sql_api(api: &mut Vec<ApiConfig>, sql: &mut Vec<ApiSql>, spec: SqlApiSpec<'_>) {
    api.push(base_api(
        &spec.id,
        &spec.path,
        spec.method,
        &spec.name,
        &spec.note,
        spec.params,
        spec.datasource_id,
        spec.group_id,
        spec.privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(spec.id),
        sql_text: Some(spec.sql_text),
        transform_plugin: Some("sql".to_string()),
        transform_plugin_params: Some(String::new()),
    });
}

fn push_query_builder_api(
    api: &mut Vec<ApiConfig>,
    sql: &mut Vec<ApiSql>,
    resource_path: &str,
    suffix: &str,
    schema: &TableSchema,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) {
    let id = format!("{}_{}", slug_id(resource_path), suffix.replace('-', "_"));
    let path = format!("{resource_path}/{suffix}");
    let select = schema.columns.iter().map(|column| column.name.clone()).collect::<Vec<_>>();
    api.push(base_api(
        &id,
        &path,
        "GET",
        &format!("{} {}", schema.table, suffix),
        "QueryBuilder page API",
        vec![
            json!({"name":"keyword","type":"string"}),
            json!({"name":"limit","type":"bigint"}),
            json!({"name":"offset","type":"bigint"}),
        ],
        datasource_id,
        group_id,
        privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(id),
        sql_text: Some(
            json!({
                "type": "queryBuilder",
                "table": schema.table,
                "select": select,
                "rules": {"combinator":"and","rules":[]},
                "orderBy": default_order(schema),
                "limit": {"param":"limit","default":20,"max":100},
                "offset": {"param":"offset","default":0},
                "count": true
            })
            .to_string(),
        ),
        transform_plugin: Some("queryBuilder".to_string()),
        transform_plugin_params: Some("resultType=page".to_string()),
    });
}

fn push_view_sql_api(
    api: &mut Vec<ApiConfig>,
    sql: &mut Vec<ApiSql>,
    resource_path: &str,
    schema: &TableSchema,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) {
    let id = format!("{}_view_sql_list", slug_id(resource_path));
    let path = format!("{resource_path}/view-sql-list");
    api.push(base_api(
        &id,
        &path,
        "GET",
        &format!("{} View SQL List", schema.table),
        "View/report/analysis API",
        vec![
            json!({"name":"columns","type":"Array<string>"}),
            json!({"name":"order_by","type":"string"}),
            json!({"name":"limit","type":"bigint"}),
            json!({"name":"offset","type":"bigint"}),
        ],
        datasource_id,
        group_id,
        privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(id.clone()),
        sql_text: Some(format!(
            "select [[ columns | ident_list ]] from {} a where 1 = 1 order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]",
            schema.table
        )),
        transform_plugin: Some("viewSql".to_string()),
        transform_plugin_params: Some("resultType=page".to_string()),
    });
    sql.push(ApiSql {
        id: None,
        api_id: Some(id),
        sql_text: Some(format!("select count(*) as total from {} a where 1 = 1", schema.table)),
        transform_plugin: Some("viewSqlCount".to_string()),
        transform_plugin_params: Some(String::new()),
    });
}

fn base_api(
    id: &str,
    path: &str,
    method: &str,
    name: &str,
    note: &str,
    params: Vec<serde_json::Value>,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) -> ApiConfig {
    ApiConfig {
        id: Some(id.to_string()),
        name: Some(name.to_string()),
        note: Some(note.to_string()),
        path: Some(path.to_string()),
        method: Some(method.to_string()),
        datasource_id: Some(datasource_id.to_string()),
        sql_list: Vec::new(),
        params: Some(serde_json::to_string(&params).unwrap()),
        status: Some(1),
        previlege: Some(privilege),
        group_id: Some(group_id.to_string()),
        cache_plugin: None,
        cache_plugin_params: None,
        create_time: None,
        update_time: None,
        content_type: Some("application/x-www-form-urlencoded".to_string()),
        open_trans: Some(0),
        json_param: None,
        alarm_plugin: None,
        alarm_plugin_param: None,
    }
}

fn normalize_resource_path(path: &str) -> anyhow::Result<String> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err(anyhow!("resource_path is required"));
    }
    Ok(trimmed.to_string())
}

fn slug_id(resource_path: &str) -> String {
    resource_path.replace('/', "_").replace('-', "_")
}

fn params_for_columns(columns: &[ColumnInfo]) -> Vec<serde_json::Value> {
    columns
        .iter()
        .map(|column| json!({"name": column.name, "type": param_type(&column.column_type)}))
        .collect()
}

fn params_for_names(names: &[&str], schema: &TableSchema) -> Vec<serde_json::Value> {
    names
        .iter()
        .filter_map(|name| schema.columns.iter().find(|column| column.name == *name))
        .map(|column| json!({"name": column.name, "type": param_type(&column.column_type)}))
        .collect()
}

fn param_type(raw: &str) -> &'static str {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("int") { "bigint" }
    else if lower.contains("double") || lower.contains("real") || lower.contains("float") || lower.contains("numeric") || lower.contains("decimal") { "double" }
    else if lower.contains("date") || lower.contains("time") { "date" }
    else { "string" }
}

fn insert_sql(table: &str, columns: &[ColumnInfo]) -> String {
    let names = columns.iter().map(|column| column.name.as_str()).collect::<Vec<_>>();
    let params = names.iter().map(|name| format!("${name}")).collect::<Vec<_>>();
    format!("insert into {table} ({}) values ({})", names.join(", "), params.join(", "))
}

fn update_sql(table: &str, primary_key: &str, columns: &[ColumnInfo]) -> String {
    let assignments = columns
        .iter()
        .map(|column| format!("{} = ${}", column.name, column.name))
        .collect::<Vec<_>>();
    format!("update {table} set {} where {primary_key} = ${primary_key}", assignments.join(", "))
}

fn default_order(schema: &TableSchema) -> Vec<serde_json::Value> {
    schema
        .columns
        .iter()
        .find(|column| column.primary_key)
        .map(|column| vec![json!({"field": column.name, "direction": "desc"})])
        .unwrap_or_default()
}

fn generate_curl_md() -> String {
    "# cURL Examples\n\nGenerated API calls use `/api/{path}`. Supply a token for private groups.\n".to_string()
}

fn generate_verify_md() -> String {
    "# Verify\n\n1. Validate generated API bundle.\n2. Apply group config.\n3. Apply API config.\n4. Generate token if APIs are private.\n5. Run cURL examples.\n".to_string()
}
```

- [ ] **Step 5: Run generator tests**

```bash
rtk cargo test manifest_generator::tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/manifest_generator.rs
rtk git commit -m "feat: generate table api bundles"
```

## Task 6: Generate SQL API Bundles

**Files:**
- Modify: `src/manifest_generator.rs`

- [ ] **Step 1: Add SQL bundle test**

Append this test to `src/manifest_generator.rs`:

```rust
#[test]
fn sql_bundle_generates_single_api_with_method_and_params() {
    let bundle = draft_sql_api_bundle(crate::manifest::DraftSqlInput {
        datasource_id: "postgres_demo".to_string(),
        resource_path: "demo/items/custom-search".to_string(),
        api_id: "demo_items_custom_search".to_string(),
        api_name: "Demo Items Custom Search".to_string(),
        group: crate::manifest::ManifestGroup {
            id: "demo_items_group".to_string(),
            name: "Demo Items".to_string(),
        },
        sql: "select id, name from demo_items where status = $status".to_string(),
        engine: "sql".to_string(),
    })
    .unwrap();

    assert_eq!(bundle.api_config.api[0].method.as_deref(), Some("GET"));
    assert_eq!(bundle.api_config.api[0].path.as_deref(), Some("demo/items/custom-search"));
    assert_eq!(bundle.api_config.sql[0].transform_plugin.as_deref(), Some("sql"));
    assert!(bundle.api_config.api[0].params.as_deref().unwrap().contains("status"));
}
```

- [ ] **Step 2: Run the failing test**

```bash
rtk cargo test manifest_generator::tests::sql_bundle_generates_single_api_with_method_and_params
```

Expected: FAIL because `draft_sql_api_bundle` is missing.

- [ ] **Step 3: Add SQL bundle implementation**

Add this function to `src/manifest_generator.rs`:

```rust
pub fn draft_sql_api_bundle(input: crate::manifest::DraftSqlInput) -> anyhow::Result<GeneratedBundle> {
    let resource_path = normalize_resource_path(&input.resource_path)?;
    let method = infer_method_from_sql(&input.sql);
    let params = extract_dollar_params(&input.sql)
        .into_iter()
        .map(|name| json!({"name": name, "type": "string"}))
        .collect::<Vec<_>>();
    let api = base_api(
        &input.api_id,
        &resource_path,
        method,
        &input.api_name,
        "Generated from SQL or agent-authored requirement",
        params,
        &input.datasource_id,
        &input.group.id,
        1,
    );
    let sql = ApiSql {
        id: None,
        api_id: Some(input.api_id),
        sql_text: Some(input.sql),
        transform_plugin: Some(input.engine),
        transform_plugin_params: Some(String::new()),
    };

    Ok(GeneratedBundle {
        manifest: DbapiManifest {
            version: MANIFEST_VERSION.to_string(),
            source: ManifestSource {
                datasource_id: input.datasource_id,
                table: None,
                primary_key: None,
                resource_path,
            },
            group_file: "api_group_config.json".to_string(),
            api_file: "api_config.json".to_string(),
            curl_file: "curl.md".to_string(),
            verify_file: "VERIFY.md".to_string(),
        },
        groups: vec![ApiGroup {
            id: Some(input.group.id),
            name: Some(input.group.name),
        }],
        api_config: ApiConfigExport {
            api: vec![api],
            sql: vec![sql],
        },
        curl_md: generate_curl_md(),
        verify_md: generate_verify_md(),
    })
}

fn infer_method_from_sql(sql: &str) -> &'static str {
    match sql.trim().split_whitespace().next().unwrap_or("").to_ascii_lowercase().as_str() {
        "select" | "with" | "show" => "GET",
        "insert" => "POST",
        "update" => "PATCH",
        "delete" => "DELETE",
        _ => "POST",
    }
}

fn extract_dollar_params(sql: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut in_single_quote = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            in_single_quote = !in_single_quote;
            continue;
        }
        if in_single_quote || ch != '$' {
            continue;
        }
        let mut name = String::new();
        while let Some(next) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                name.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if !name.is_empty() && !params.contains(&name) {
            params.push(name);
        }
    }
    params
}
```

- [ ] **Step 4: Run SQL bundle tests**

```bash
rtk cargo test manifest_generator::tests::sql_bundle_generates_single_api_with_method_and_params
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/manifest_generator.rs
rtk git commit -m "feat: generate sql api bundles"
```

## Task 7: Write and Read Bundle Files

**Files:**
- Modify: `src/bundle_files.rs`

- [ ] **Step 1: Add file round-trip test**

Replace `src/bundle_files.rs` with tests and keep the temporary command function at the bottom:

```rust
use crate::cli::{BundleArgs, BundleCommand};
use crate::manifest::GeneratedBundle;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DbapiManifest, GeneratedBundle, ManifestSource, MANIFEST_VERSION};
    use crate::model::ApiConfigExport;

    #[test]
    fn writes_bundle_files() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = GeneratedBundle {
            manifest: DbapiManifest {
                version: MANIFEST_VERSION.to_string(),
                source: ManifestSource {
                    datasource_id: "ds".to_string(),
                    table: Some("demo_items".to_string()),
                    primary_key: Some("id".to_string()),
                    resource_path: "demo/items".to_string(),
                },
                group_file: "api_group_config.json".to_string(),
                api_file: "api_config.json".to_string(),
                curl_file: "curl.md".to_string(),
                verify_file: "VERIFY.md".to_string(),
            },
            groups: vec![],
            api_config: ApiConfigExport { api: vec![], sql: vec![] },
            curl_md: "# curls\n".to_string(),
            verify_md: "# verify\n".to_string(),
        };

        write_bundle(dir.path(), &bundle).unwrap();

        assert!(dir.path().join("dbapi_manifest.json").exists());
        assert!(dir.path().join("api_group_config.json").exists());
        assert!(dir.path().join("api_config.json").exists());
        assert!(dir.path().join("curl.md").exists());
        assert!(dir.path().join("VERIFY.md").exists());
    }
}
```

- [ ] **Step 2: Implement file helpers**

Add above `run_bundle_command`:

```rust
pub fn write_bundle(dir: &Path, bundle: &GeneratedBundle) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_json(&dir.join("dbapi_manifest.json"), &bundle.manifest)?;
    write_json(&dir.join("api_group_config.json"), &bundle.groups)?;
    write_json(&dir.join("api_config.json"), &bundle.api_config)?;
    std::fs::write(dir.join("curl.md"), &bundle.curl_md)?;
    std::fs::write(dir.join("VERIFY.md"), &bundle.verify_md)?;
    Ok(())
}

pub fn read_group_file(dir: &Path) -> anyhow::Result<Vec<crate::model::ApiGroup>> {
    read_json(&dir.join("api_group_config.json"))
}

pub fn read_api_file(dir: &Path) -> anyhow::Result<crate::model::ApiConfigExport> {
    read_json(&dir.join("api_config.json"))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
```

- [ ] **Step 3: Run file tests**

```bash
rtk cargo test bundle_files::tests::writes_bundle_files
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
rtk git add src/bundle_files.rs
rtk git commit -m "feat: write dbapi bundle files"
```

## Task 8: Implement DBAPI HTTP Client

**Files:**
- Modify: `src/dbapi_client.rs`

- [ ] **Step 1: Add client URL tests**

Replace `src/dbapi_client.rs` with:

```rust
use crate::manifest::ValidationReport;
use crate::model::{ApiConfigExport, ApiGroup, AppInfo, DataSource};
use crate::schema::TableSchema;
use reqwest::multipart;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::path::Path;

#[derive(Clone)]
pub struct DbapiClient {
    base_url: String,
    http: reqwest::Client,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_base_url_and_path() {
        let client = DbapiClient::new("http://127.0.0.1:8520/").unwrap();
        assert_eq!(client.url("/datasource/getAll"), "http://127.0.0.1:8520/datasource/getAll");
    }
}
```

- [ ] **Step 2: Implement constructor and URL helper**

Add:

```rust
impl DbapiClient {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            anyhow::bail!("base_url is required");
        }
        Ok(Self {
            base_url,
            http: reqwest::Client::new(),
        })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}
```

- [ ] **Step 3: Add read and apply methods**

Append:

```rust
impl DbapiClient {
    pub async fn list_datasources(&self) -> anyhow::Result<Vec<DataSource>> {
        self.post_json("/datasource/getAll", &json!({})).await
    }

    pub async fn inspect_table_schema(&self, datasource_id: &str, table: &str) -> anyhow::Result<TableSchema> {
        let columns = self
            .post_json(
                "/table/getAllColumns",
                &json!({"datasourceId": datasource_id, "table": table}),
            )
            .await?;
        Ok(TableSchema {
            table: table.to_string(),
            columns,
        })
    }

    pub async fn import_groups_file(&self, path: &Path) -> anyhow::Result<()> {
        self.upload_file("/apiConfig/importGroup", path).await
    }

    pub async fn import_api_file(&self, path: &Path) -> anyhow::Result<()> {
        self.upload_file("/apiConfig/import", path).await
    }

    pub async fn create_app_token_for_group(
        &self,
        app_name: &str,
        group_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let app: AppInfo = self
            .post_json(
                "/app/create",
                &json!({"name": app_name, "note": "Generated by DBAPI bundle workflow", "expireDesc": "forever"}),
            )
            .await?;
        let app_id = app.id.as_deref().ok_or_else(|| anyhow::anyhow!("app id missing"))?;
        let secret = app.secret.as_deref().ok_or_else(|| anyhow::anyhow!("app secret missing"))?;
        let _: serde_json::Value = self
            .post_json("/app/auth", &json!({"appId": app_id, "groupIds": group_id}))
            .await?;
        self.get_json(&format!("/token/generate?appid={app_id}&secret={secret}")).await
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> anyhow::Result<T> {
        let response = self.http.post(self.url(path)).json(body).send().await?;
        parse_response(response).await
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = self.http.get(self.url(path)).send().await?;
        parse_response(response).await
    }

    async fn upload_file(&self, path: &str, file_path: &Path) -> anyhow::Result<()> {
        let bytes = tokio::fs::read(file_path).await?;
        let filename = file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("bundle.json")
            .to_string();
        let part = multipart::Part::bytes(bytes).file_name(filename);
        let form = multipart::Form::new().part("file", part);
        let response = self.http.post(self.url(path)).multipart(form).send().await?;
        let _: serde_json::Value = parse_response(response).await?;
        Ok(())
    }
}

async fn parse_response<T: DeserializeOwned>(response: reqwest::Response) -> anyhow::Result<T> {
    let status = response.status();
    let value: serde_json::Value = response.json().await?;
    if !status.is_success() {
        anyhow::bail!("request failed with status {}: {}", status, value);
    }
    if let Some(success) = value.get("success").and_then(serde_json::Value::as_bool) {
        if !success {
            anyhow::bail!(
                "{}",
                value.get("msg").and_then(serde_json::Value::as_str).unwrap_or("DBAPI request failed")
            );
        }
        return Ok(serde_json::from_value(value.get("data").cloned().unwrap_or(serde_json::Value::Null))?);
    }
    Ok(serde_json::from_value(value)?)
}
```

- [ ] **Step 4: Run client tests**

```bash
rtk cargo test dbapi_client::tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/dbapi_client.rs
rtk git commit -m "feat: add dbapi management http client"
```

## Task 9: Implement Bundle Validation

**Files:**
- Modify: `src/manifest_validator.rs`

- [ ] **Step 1: Add validation tests**

Create `src/manifest_validator.rs`:

```rust
use crate::manifest::ValidationReport;
use crate::model::{ApiConfigExport, ApiGroup, DataSource};
use crate::schema::TableSchema;
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiConfig, ApiConfigExport};

    #[test]
    fn rejects_leading_slash_and_duplicate_paths() {
        let mut bundle = ApiConfigExport { api: vec![], sql: vec![] };
        bundle.api.push(api("one", "/demo/items/get"));
        bundle.api.push(api("two", "/demo/items/get"));

        let report = validate_bundle_shape(&[], &bundle);

        assert!(!report.success);
        assert!(report.errors.iter().any(|error| error.contains("must not start with /")));
        assert!(report.errors.iter().any(|error| error.contains("duplicate api path")));
    }

    fn api(id: &str, path: &str) -> ApiConfig {
        ApiConfig {
            id: Some(id.to_string()),
            name: Some(id.to_string()),
            note: None,
            path: Some(path.to_string()),
            method: Some("GET".to_string()),
            datasource_id: Some("ds".to_string()),
            sql_list: Vec::new(),
            params: Some("[]".to_string()),
            status: Some(1),
            previlege: Some(1),
            group_id: Some("group".to_string()),
            cache_plugin: None,
            cache_plugin_params: None,
            create_time: None,
            update_time: None,
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            open_trans: Some(0),
            json_param: None,
            alarm_plugin: None,
            alarm_plugin_param: None,
        }
    }
}
```

- [ ] **Step 2: Implement shape validation**

Add above the test module:

```rust
pub fn validate_bundle_shape(groups: &[ApiGroup], bundle: &ApiConfigExport) -> ValidationReport {
    let mut report = ValidationReport::default();
    let mut group_ids = HashSet::new();
    let mut api_ids = HashSet::new();
    let mut paths = HashSet::new();

    for group in groups {
        let id = group.id.as_deref().unwrap_or("").trim();
        let name = group.name.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            report.error("group id is required");
        }
        if name.is_empty() {
            report.error("group name is required");
        }
        if !id.is_empty() && !group_ids.insert(id.to_string()) {
            report.error(format!("duplicate group id in bundle: {id}"));
        }
    }

    for api in &bundle.api {
        let id = api.id.as_deref().unwrap_or("").trim();
        let path = api.path.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            report.error("api id is required");
        }
        if path.is_empty() {
            report.error("api path is required");
        }
        if path.starts_with('/') {
            report.error(format!("api path must not start with /: {path}"));
        }
        if !id.is_empty() && !api_ids.insert(id.to_string()) {
            report.error(format!("duplicate api id in bundle: {id}"));
        }
        if !path.is_empty() && !paths.insert(path.to_string()) {
            report.error(format!("duplicate api path in bundle: {path}"));
        }
        if api.datasource_id.as_deref().unwrap_or("").trim().is_empty() {
            report.error(format!("api datasourceId is required: {id}"));
        }
        if api.method.as_deref().unwrap_or("").trim().is_empty() {
            report.error(format!("api method is required: {id}"));
        }
    }

    for row in &bundle.sql {
        let api_id = row.api_id.as_deref().unwrap_or("").trim();
        if api_id.is_empty() {
            report.error("sql apiId is required");
        } else if !api_ids.contains(api_id) {
            report.error(format!("sql references unknown api id: {api_id}"));
        }
    }

    report
}
```

- [ ] **Step 3: Add client-backed validation function**

Add:

```rust
pub async fn validate_against_server(
    client: &crate::dbapi_client::DbapiClient,
    groups: &[ApiGroup],
    bundle: &ApiConfigExport,
) -> anyhow::Result<ValidationReport> {
    let mut report = validate_bundle_shape(groups, bundle);
    let datasources = client.list_datasources().await?;
    let datasource_ids = datasources
        .iter()
        .filter_map(|ds| ds.id.as_deref())
        .collect::<HashSet<_>>();

    for api in &bundle.api {
        if let Some(datasource_id) = api.datasource_id.as_deref() {
            if !datasource_ids.contains(datasource_id) {
                report.error(format!("datasource does not exist: {datasource_id}"));
            }
        }
    }

    Ok(report)
}
```

- [ ] **Step 4: Run validator tests**

```bash
rtk cargo test manifest_validator::tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/manifest_validator.rs
rtk git commit -m "feat: validate dbapi bundles"
```

## Task 10: Wire CLI Draft, Validate, and Apply

**Files:**
- Modify: `src/bundle_files.rs`

- [ ] **Step 1: Replace bundle command runner**

Update `run_bundle_command` in `src/bundle_files.rs`:

```rust
pub async fn run_bundle_command(args: BundleArgs) -> anyhow::Result<()> {
    match args.command {
        BundleCommand::DraftTable(args) => {
            let client = crate::dbapi_client::DbapiClient::new(args.base_url)?;
            let schema = client.inspect_table_schema(&args.datasource_id, &args.table).await?;
            let bundle = crate::manifest_generator::draft_table_crud_bundle(
                crate::manifest::DraftTableInput {
                    datasource_id: args.datasource_id,
                    table: args.table,
                    primary_key: args.primary_key,
                    resource_path: args.resource_path,
                    group: crate::manifest::ManifestGroup {
                        id: args.group_id,
                        name: args.group_name,
                    },
                    public: true,
                },
                &schema,
            )?;
            write_bundle(&args.out, &bundle)?;
            println!("bundle written to {}", args.out.display());
            Ok(())
        }
        BundleCommand::DraftSql(args) => {
            let bundle = crate::manifest_generator::draft_sql_api_bundle(crate::manifest::DraftSqlInput {
                datasource_id: args.datasource_id,
                resource_path: args.resource_path,
                api_id: args.api_id,
                api_name: args.api_name,
                group: crate::manifest::ManifestGroup {
                    id: args.group_id,
                    name: args.group_name,
                },
                sql: args.sql,
                engine: args.engine,
            })?;
            write_bundle(&args.out, &bundle)?;
            println!("bundle written to {}", args.out.display());
            Ok(())
        }
        BundleCommand::Validate(args) => {
            let client = crate::dbapi_client::DbapiClient::new(args.base_url)?;
            let groups = read_group_file(&args.dir)?;
            let api = read_api_file(&args.dir)?;
            let report = crate::manifest_validator::validate_against_server(&client, &groups, &api).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.success { Ok(()) } else { anyhow::bail!("bundle validation failed") }
        }
        BundleCommand::Apply(args) => {
            if !args.allow_write {
                anyhow::bail!("apply requires --allow-write=true");
            }
            let client = crate::dbapi_client::DbapiClient::new(args.base_url)?;
            let groups = read_group_file(&args.dir)?;
            let api = read_api_file(&args.dir)?;
            let report = crate::manifest_validator::validate_against_server(&client, &groups, &api).await?;
            if !report.success {
                println!("{}", serde_json::to_string_pretty(&report)?);
                anyhow::bail!("bundle validation failed");
            }
            client.import_groups_file(&args.dir.join("api_group_config.json")).await?;
            client.import_api_file(&args.dir.join("api_config.json")).await?;
            println!("bundle applied from {}", args.dir.display());
            Ok(())
        }
    }
}
```

- [ ] **Step 2: Verify CLI shape**

Run:

```bash
rtk cargo run -- bundle draft-sql \
  --datasource-id local_sqlite_demo \
  --resource-path demo/items/custom-search \
  --api-id demo_items_custom_search \
  --api-name "Demo Items Custom Search" \
  --group-id demo_items_group \
  --group-name "Demo Items" \
  --sql "select id, name from demo_items where status = $status" \
  --out target/dbapi-plan-smoke
```

Expected: command exits successfully and creates `target/dbapi-plan-smoke/api_config.json`.

- [ ] **Step 3: Run unit tests**

```bash
rtk cargo test bundle_files::tests manifest_generator::tests manifest_validator::tests dbapi_client::tests
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
rtk git add src/bundle_files.rs
rtk git commit -m "feat: wire bundle cli workflow"
```

## Task 11: Add Repo-Local Skills

**Files:**
- Create: `skills/dbapi-generate-table-apis/SKILL.md`
- Create: `skills/dbapi-generate-sql-api/SKILL.md`
- Create: `skills/dbapi-apply-api-bundle/SKILL.md`
- Create: `skills/dbapi-token-workflow/SKILL.md`
- Create: `skills/dbapi-export-import-workflow/SKILL.md`
- Modify: `skills/dbapi-demo-crud/SKILL.md`

- [ ] **Step 1: Create `dbapi-generate-table-apis`**

Create `skills/dbapi-generate-table-apis/SKILL.md`:

````markdown
---
name: dbapi-generate-table-apis
description: Use when generating DBAPI CRUD, QueryBuilder list/table, and View SQL report APIs from a datasource table. Requires datasource_id, table, resource_path, group id/name, and primary_key when schema metadata cannot prove one.
---

# DBAPI Generate Table APIs

## Purpose

Generate reviewable DBAPI bundle files before applying anything.

## Required Inputs

- `base_url`, normally `http://127.0.0.1:8520`
- `datasource_id`
- `table`
- `resource_path`
- `group_id`
- `group_name`
- `primary_key` when introspection does not expose one

Do not guess `resource_path` from the table name. Ask the user if it is missing.

## Engine Rules

- `create`, `get`, `update`, `delete` use SQL.
- `qb-list` and `table` use QueryBuilder.
- `view-sql-list` uses View SQL plus View SQL Count.

## Command

```bash
rtk cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id "$DATASOURCE_ID" \
  --table "$TABLE" \
  --primary-key "$PRIMARY_KEY" \
  --resource-path "$RESOURCE_PATH" \
  --group-id "$GROUP_ID" \
  --group-name "$GROUP_NAME" \
  --out "target/dbapi-bundles/$GROUP_ID"
```

## Generated Files

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`

After generation, use `dbapi-apply-api-bundle` for validation and apply.
````

- [ ] **Step 2: Create `dbapi-generate-sql-api`**

Create `skills/dbapi-generate-sql-api/SKILL.md`:

````markdown
---
name: dbapi-generate-sql-api
description: Use when generating a DBAPI API from SQL or from a user requirement that the agent has already converted into SQL or View SQL.
---

# DBAPI Generate SQL API

## Purpose

Generate one reviewable API bundle from SQL. For natural language requirements, first write the SQL or View SQL template, then generate the bundle.

## Rules

- Use bind parameters such as `$status`.
- Query/report APIs should prefer View SQL when dynamic columns, ordering, joins, or analysis are required.
- Simple one-off statements can use SQL.
- Do not write directly to DBAPI from this skill.

## Command

```bash
rtk cargo run -- bundle draft-sql \
  --datasource-id "$DATASOURCE_ID" \
  --resource-path "$RESOURCE_PATH" \
  --api-id "$API_ID" \
  --api-name "$API_NAME" \
  --group-id "$GROUP_ID" \
  --group-name "$GROUP_NAME" \
  --sql "$SQL_TEXT" \
  --engine sql \
  --out "target/dbapi-bundles/$API_ID"
```

Then validate with `dbapi-apply-api-bundle`.
````

- [ ] **Step 3: Create apply/token/export skills**

Create `skills/dbapi-apply-api-bundle/SKILL.md`:

````markdown
---
name: dbapi-apply-api-bundle
description: Use when validating or applying generated DBAPI bundle files through the local DBAPI HTTP management API.
---

# DBAPI Apply API Bundle

## Validate

```bash
rtk cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR"
```

Expected success output has `"success": true`.

## Apply

Only apply after the user confirms the generated files.

```bash
rtk cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR" \
  --allow-write=true
```

After apply, read `curl.md` and run the listed smoke commands.
````

Create `skills/dbapi-token-workflow/SKILL.md`:

````markdown
---
name: dbapi-token-workflow
description: Use when creating a DBAPI app, authorizing API groups, generating a token, and testing private APIs with curl.
---

# DBAPI Token Workflow

## Commands

```bash
APP_JSON=$(rtk curl -sS -X POST \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode "name=$APP_NAME" \
  --data-urlencode 'note=Generated by DBAPI token workflow' \
  --data-urlencode 'expireDesc=forever' \
  'http://127.0.0.1:8520/app/create')

APP_ID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "$APP_JSON")
SECRET=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["secret"])' "$APP_JSON")

rtk curl -sS -X POST \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode "appId=$APP_ID" \
  --data-urlencode "groupIds=$GROUP_ID" \
  'http://127.0.0.1:8520/app/auth'

TOKEN_JSON=$(rtk curl -sS "http://127.0.0.1:8520/token/generate?appid=$APP_ID&secret=$SECRET")
TOKEN=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["token"])' "$TOKEN_JSON")

printf 'APP_ID=%s\nTOKEN=%s\n' "$APP_ID" "$TOKEN"
```

Never commit generated tokens.
````

Create `skills/dbapi-export-import-workflow/SKILL.md`:

````markdown
---
name: dbapi-export-import-workflow
description: Use when exporting or importing DBAPI API groups and API configs through the management API.
---

# DBAPI Export Import Workflow

## Export APIs

```bash
rtk curl -sS -X POST \
  -o api_config.json \
  "http://127.0.0.1:8520/apiConfig/downloadConfig?ids=$API_IDS"
```

## Export Groups

```bash
rtk curl -sS -X POST \
  -o api_group_config.json \
  "http://127.0.0.1:8520/apiConfig/downloadGroupConfig?ids=$GROUP_IDS"
```

## Import Groups

```bash
rtk curl -sS -X POST \
  -F "file=@api_group_config.json" \
  "http://127.0.0.1:8520/apiConfig/importGroup"
```

## Import APIs

```bash
rtk curl -sS -X POST \
  -F "file=@api_config.json" \
  "http://127.0.0.1:8520/apiConfig/import"
```
````

- [ ] **Step 4: Update old demo skill header**

Edit `skills/dbapi-demo-crud/SKILL.md` purpose section to say:

```markdown
Use this skill only for the historical local demo CRUD seed. For new API creation, use `dbapi-generate-table-apis`, `dbapi-generate-sql-api`, and `dbapi-apply-api-bundle`.
```

- [ ] **Step 5: Verify skill content**

```bash
rtk rg -n "demo/items/list|qb-list|view-sql-list|bundle draft-table|bundle apply|token/generate" skills
```

Expected: output includes the new skill commands and the old demo warning.

- [ ] **Step 6: Commit**

```bash
rtk git add skills
rtk git commit -m "docs: add dbapi api generation skills"
```

## Task 12: Implement MCP HTTP Sidecar

**Files:**
- Modify: `src/mcp_server.rs`

- [ ] **Step 1: Add tool request/response structs**

Replace `src/mcp_server.rs` with:

```rust
use crate::cli::McpArgs;
use crate::manifest::{DraftSqlInput, DraftTableInput, ValidationReport};
use rmcp::{ErrorData as McpError, ServerHandler, model::*, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct DbapiMcpServer {
    client: crate::dbapi_client::DbapiClient,
    allow_write: bool,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectTableRequest {
    pub datasource_id: String,
    pub table: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidateBundleRequest {
    pub dir: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyBundleRequest {
    pub dir: String,
    pub allow_write: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextResult {
    message: String,
}
```

- [ ] **Step 2: Implement server handler and tools**

Append:

```rust
#[tool_router]
impl DbapiMcpServer {
    pub fn new(client: crate::dbapi_client::DbapiClient, allow_write: bool) -> Self {
        Self {
            client,
            allow_write,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List DBAPI datasources")]
    async fn list_datasources(&self) -> Result<CallToolResult, McpError> {
        json_result(self.client.list_datasources().await)
    }

    #[tool(description = "Inspect a DBAPI datasource table schema")]
    async fn inspect_table_schema(&self, #[tool(aggr)] request: InspectTableRequest) -> Result<CallToolResult, McpError> {
        json_result(self.client.inspect_table_schema(&request.datasource_id, &request.table).await)
    }

    #[tool(description = "Validate generated DBAPI bundle files in a directory")]
    async fn validate_api_bundle(&self, #[tool(aggr)] request: ValidateBundleRequest) -> Result<CallToolResult, McpError> {
        let dir = std::path::PathBuf::from(request.dir);
        let groups = crate::bundle_files::read_group_file(&dir).map_err(tool_error)?;
        let api = crate::bundle_files::read_api_file(&dir).map_err(tool_error)?;
        json_result(crate::manifest_validator::validate_against_server(&self.client, &groups, &api).await)
    }

    #[tool(description = "Apply generated DBAPI bundle files after explicit write confirmation")]
    async fn apply_api_config_bundle(&self, #[tool(aggr)] request: ApplyBundleRequest) -> Result<CallToolResult, McpError> {
        if !self.allow_write || !request.allow_write {
            return json_result(Ok(ValidationReport {
                success: false,
                errors: vec!["apply requires process --allow-write=true and tool allow_write=true".to_string()],
                warnings: vec![],
            }));
        }
        let dir = std::path::PathBuf::from(request.dir);
        self.client.import_groups_file(&dir.join("api_group_config.json")).await.map_err(tool_error)?;
        self.client.import_api_file(&dir.join("api_config.json")).await.map_err(tool_error)?;
        json_result(Ok(TextResult { message: "bundle applied".to_string() }))
    }
}

#[tool_handler]
impl ServerHandler for DbapiMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "db-api-rs-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("DBAPI MCP sidecar. Draft and validate by default; writes require explicit enablement.".to_string()),
        }
    }
}

fn json_result<T: Serialize>(result: anyhow::Result<T>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(value) => Ok(CallToolResult::success(vec![Content::json(value).map_err(tool_error)?])),
        Err(error) => Err(tool_error(error)),
    }
}

fn tool_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}
```

- [ ] **Step 3: Implement HTTP transport**

Append:

```rust
pub async fn serve(args: McpArgs) -> anyhow::Result<()> {
    if args.transport != "http" {
        anyhow::bail!("only --transport http is supported in this build");
    }
    let client = crate::dbapi_client::DbapiClient::new(args.base_url)?;
    let server = DbapiMcpServer::new(client, args.allow_write);
    let cancellation = tokio_util::sync::CancellationToken::new();
    let service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        move || Ok(server.clone()),
        Default::default(),
        cancellation.clone(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&args.listen).await?;
    tracing::info!("db-api-rs MCP sidecar listening on {}", args.listen);
    axum::serve(listener, router).await?;
    Ok(())
}
```

- [ ] **Step 4: Compile MCP server**

```bash
rtk cargo check
```

Expected: PASS. If rmcp type names differ in the installed version, update imports and `StreamableHttpService` construction to match the examples for the selected `rmcp` crate version, then rerun `rtk cargo check`.

- [ ] **Step 5: Commit**

```bash
rtk git add src/mcp_server.rs Cargo.toml Cargo.lock
rtk git commit -m "feat: add dbapi mcp http sidecar"
```

## Task 13: Docker Compose and README User/Agent Docs

**Files:**
- Modify: `Dockerfile`
- Modify: `docker-compose.yml`
- Modify: `README.md`

- [ ] **Step 1: Expose sidecar port**

Change the `Dockerfile` expose line to:

```dockerfile
EXPOSE 8520 8521
```

- [ ] **Step 2: Add MCP sidecar service**

In `docker-compose.yml`, keep the existing `db-api-rs` service and add:

```yaml
  dbapi-mcp:
    build:
      context: .
      dockerfile: Dockerfile
    command:
      - /app/db-api-rs
      - mcp
      - --transport
      - http
      - --listen
      - 0.0.0.0:8521
      - --base-url
      - http://db-api-rs:8520
      - --allow-write=false
    ports:
      - "127.0.0.1:8521:8521"
    depends_on:
      db-api-rs:
        condition: service_started
```

Also change the existing `db-api-rs` published port to:

```yaml
ports:
  - "127.0.0.1:8520:8520"
```

- [ ] **Step 3: Add a README quick index for humans and agents**

Near the top of `README.md`, after the feature list and before `View SQL Templates`, add:

```markdown
## AI and Agent Workflows

DBAPI includes an agent-friendly workflow for creating APIs without manually filling every UI field.

Use these entrypoints:

- Humans: start with `Quick Start`, then use the web UI or the bundle commands below.
- Local coding agents: read this README, then use repo-local skills under `skills/`.
- MCP-capable agents: connect to the MCP sidecar on `127.0.0.1:8521`.

Repo-local skills:

- `skills/dbapi-generate-table-apis`: generate CRUD, QueryBuilder list/table, and View SQL report APIs from a datasource table.
- `skills/dbapi-generate-sql-api`: generate an API from SQL or a requirement translated into SQL/View SQL.
- `skills/dbapi-apply-api-bundle`: validate and apply generated bundle files.
- `skills/dbapi-token-workflow`: create app, authorize group, generate token, and verify private APIs.
- `skills/dbapi-export-import-workflow`: export/import API groups and API configs.

Generated API paths must use explicit `resource_path`; do not guess paths from table names.
```

- [ ] **Step 4: Add README bundle workflow section**

Append to `README.md`:

````markdown
## DBAPI Bundle Workflow

DBAPI supports a file-first workflow for AI-generated API groups and APIs. The generator writes reviewable files first, then validation checks them before anything is applied.

1. Generate reviewable files:
   - `dbapi_manifest.json`
   - `api_group_config.json`
   - `api_config.json`
   - `curl.md`
   - `VERIFY.md`
2. Validate the bundle.
3. Apply only after confirmation.

Example:

```bash
cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id postgres_demo \
  --table demo_items \
  --primary-key id \
  --resource-path demo/items \
  --group-id demo_items_group \
  --group-name "Demo Items" \
  --out target/dbapi-bundles/demo_items

cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items
```

Apply only after reviewing the generated files:

```bash
cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items \
  --allow-write=true
```

Default table generation creates:

| API | Method | Engine |
| --- | --- | --- |
| `{resource_path}/create` | POST | SQL |
| `{resource_path}/get` | GET | SQL |
| `{resource_path}/update` | PATCH | SQL |
| `{resource_path}/delete` | DELETE | SQL |
| `{resource_path}/qb-list` | GET | QueryBuilder |
| `{resource_path}/table` | GET | QueryBuilder |
| `{resource_path}/view-sql-list` | GET | View SQL |
````

- [ ] **Step 5: Add README MCP sidecar section**

Append to `README.md` after the bundle workflow:

````markdown
## MCP Sidecar

Docker Compose also starts an MCP HTTP sidecar on `127.0.0.1:8521`:

```bash
docker compose up -d --build
```

The sidecar defaults to read/draft/validate mode. Writes require starting it with `--allow-write=true` and passing `allow_write=true` to write tools.

Available MCP tools include:

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `apply_api_config_bundle`

The sidecar calls DBAPI's existing HTTP management routes and does not directly read or write `data.db`.
````

- [ ] **Step 6: Run compose config and README checks**

```bash
rtk docker compose config
rtk rg -n "AI and Agent Workflows|DBAPI Bundle Workflow|MCP Sidecar|dbapi-generate-table-apis|allow-write" README.md
```

Expected: compose config passes and README search output includes all listed headings and skill/write-safety terms.

- [ ] **Step 7: Commit**

```bash
rtk git add Dockerfile docker-compose.yml README.md
rtk git commit -m "docs: document dbapi agent workflows"
```

## Task 14: End-to-End Verification

**Files:**
- No source file changes unless a previous task has a concrete failure to fix.

- [ ] **Step 1: Run Rust tests**

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests and build**

```bash
rtk npm --prefix frontend test -- --run
rtk npm --prefix frontend run build
```

Expected: PASS.

- [ ] **Step 3: Start Docker Compose**

```bash
rtk docker compose up -d --build
```

Expected: `db-api-rs`, `dbapi-mcp`, and `postgres` containers start.

- [ ] **Step 4: Verify DBAPI health**

```bash
rtk curl -sS http://127.0.0.1:8520/health
```

Expected:

```text
OK
```

- [ ] **Step 5: Verify MCP HTTP endpoint is reachable**

```bash
rtk curl -sS -i http://127.0.0.1:8521/mcp
```

Expected: HTTP response from the MCP sidecar. A protocol-level method error is acceptable for a raw curl GET; connection refused is not acceptable.

- [ ] **Step 6: Verify CLI draft and validate**

```bash
rtk cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id postgres_demo \
  --table demo_items \
  --primary-key id \
  --resource-path plan/demo/items \
  --group-id plan_demo_items_group \
  --group-name "Plan Demo Items" \
  --out target/dbapi-bundles/plan-demo-items

rtk cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/plan-demo-items
```

Expected: generated files exist and validation prints `"success": true`.

- [ ] **Step 7: Verify write guard**

```bash
rtk cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/plan-demo-items
```

Expected: FAIL with `apply requires --allow-write=true`.

- [ ] **Step 8: Handle verification failures**

If verification finds a source issue, do not make a catch-all final commit. Return to the task that introduced the failing behavior, apply the fix there, rerun that task's tests, and commit the exact files in that task using that task's commit command.

If no source fix is required, do not create an empty commit.

## Task 15: Push Main

**Files:**
- No file changes.

- [ ] **Step 1: Check working tree**

```bash
rtk git status --short --branch
```

Expected: `main` may show only pre-existing `data.db` and `data.db-shm` local changes. Do not include those files unless the user explicitly asks.

- [ ] **Step 2: Push direct to main**

```bash
rtk git push origin main
```

Expected: push succeeds.

- [ ] **Step 3: Confirm alignment**

```bash
rtk git log -1 --oneline --decorate --all --branches=main --remotes=origin/main
rtk git status --short --branch
```

Expected: `HEAD`, `origin/main`, and `origin/HEAD` point to the final implementation commit. Local runtime data changes can remain unstaged.
