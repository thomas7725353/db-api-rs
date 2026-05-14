# db-api-rs

`db-api-rs` is a standalone Rust runtime and React admin UI for publishing database queries as HTTP APIs. It is designed for fast internal API creation over SQLite, MySQL, and PostgreSQL data sources.

The runtime lives at the repository root and is implemented with Axum, SeaORM/SQLx, SQLParser, SeaQuery, MiniJinja, React, and Ant Design.

## Features

- Manage data sources from the web UI.
- Create and publish API endpoints under `/api/{path}`.
- Support public APIs and token-protected APIs.
- Record access logs for monitoring.
- Build APIs in three modes:
  - `QueryBuilder`: structured query DSL for common list/filter/count APIs.
  - `SQL`: hand-written SQL with `$param` bind parameters.
  - `View SQL`: complex SQL templates for joins, dynamic select columns, sort columns, limit, and offset.
- Return response modes:
  - `list`: return an array.
  - `page`: return `{ list, total, limit, offset }`.
  - `object`: return a single row object.
  - `count`: return only the total.

## AI and Agent Workflows

Humans should start with [Quick Start](#quick-start), then use the web UI or the DBAPI bundle commands below.

Local coding agents should read this README first and use the repo-local skills under `skills/` for repeatable API work:

- `dbapi-generate-table-apis`
- `dbapi-generate-sql-api`
- `dbapi-apply-api-bundle`
- `dbapi-token-workflow`
- `dbapi-export-import-workflow`

MCP-capable agents can connect to the sidecar at `http://127.0.0.1:8521/mcp`.

The MCP and CLI surfaces are intentionally aligned: use CLI for humans and CI, use MCP for agents, and use `skills/` as workflow recipes that explain how to combine the tools safely.

When generating table APIs, always use the provided `resource_path` exactly. Do not guess paths from table names. If API creation, import/export, token handling, bundle generation, or apply behavior changes, update the repo-local skills in the same change so agents keep using the current workflow.

## View SQL Templates

View SQL uses MiniJinja with custom `[[ ... ]]` delimiters for safe SQL structure fragments. Normal values should still use `$param` bind parameters.

Example:

```sql
select [[ columns | ident_list ]]
from demo_items a
inner join demo_items b
  on a.id >= b.id
where b.status = $status
order by [[ order_by | ident ]] desc
limit [[ limit | int(default=10,max=1000) ]]
offset [[ offset | int(default=0) ]]
```

Preview parameters:

```json
{
  "columns": ["a.id", "a.name", "a.status"],
  "order_by": "a.id",
  "limit": 10,
  "offset": 0,
  "status": "active"
}
```

Rendered SQL:

```sql
select a.id, a.name, a.status
from demo_items a
inner join demo_items b
  on a.id >= b.id
where b.status = ?
order by a.id desc
limit 10
offset 0
```

Supported filters:

- `ident`: a safe identifier such as `id`, `a.id`, or `a.*`.
- `ident_list`: an array or comma-separated list of safe identifiers.
- `int(default=...,max=...,min=...)`: an integer fragment with optional bounds.

Table names are intentionally not templated. Keep table names in the SQL text or in explicit API configuration.

## Quick Start

Build and run with Docker Compose:

```bash
docker compose up -d --build
```

Open:

```text
http://127.0.0.1:8520
```

Health check:

```bash
curl http://127.0.0.1:8520/health
```

The default compose file mounts the repository `data.db` into the container:

```yaml
volumes:
  - ./data.db:/data/data.db
```

## Local Development

Run the Rust backend:

```bash
cargo run
```

Run the frontend dev server:

```bash
cd frontend
npm install
npm run dev
```

Build frontend static assets:

```bash
cd frontend
npm run build
```

Run checks:

```bash
cargo test

cd frontend
npm test -- --run
npm run build
```

## Project Layout

```text
src/                    Rust runtime source
frontend/               React admin UI
static/                 Built frontend assets served by the runtime
seed_demo_api.sql       Demo API seed SQL
data.db                 Local SQLite metadata and demo database
Dockerfile              Container image build
docker-compose.yml      Local container deployment
docs/index.md           GitHub Pages project usage documentation
```

## API Execution Notes

- SQL value parameters use `$name` in stored SQL and are bound by the backend.
- View SQL structure parameters are limited to safe identifiers and integers.
- Multiple SQL statements are rejected by the runtime SQL transformer.
- QueryBuilder uses SeaQuery to generate SQL for the selected database backend.
- Published APIs have a configured HTTP method. New query APIs should use `GET`; write APIs should use `POST`, `PUT`, `PATCH`, or `DELETE`.
- `GET` requests only read URL query parameters and are rejected if the configured SQL is not a query.
- Access logs are written for successful and failed `/api/{path}` calls.

## DBAPI Bundle Workflow

The bundle workflow is file-first: generate reviewable files, validate them, and apply only after confirmation. Generated bundle directories contain:

- `dbapi_manifest.json`: bundle metadata and generated API entries.
- `api_group_config.json`: API group import payload.
- `api_config.json`: API config import payload.
- `curl.md`: example calls for the generated endpoints.
- `VERIFY.md`: validation and manual verification checklist.

Draft table APIs:

```bash
cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id postgres_demo \
  --table demo_items \
  --resource-path pg/demo/items \
  --group-id demo_items_group \
  --group-name "PG Demo Items" \
  --out bundles/pg-demo-items
```

`--primary-key` is optional. Add `--primary-key id` only when metadata cannot infer the primary key or when you need to override it.

Validate before apply:

```bash
cargo run -- bundle validate --dir bundles/pg-demo-items
```

Apply only after human confirmation:

```bash
cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir bundles/pg-demo-items \
  --allow-write
```

Default table generation creates these API paths under the chosen `resource_path`:

| Path | Method | Engine |
| --- | --- | --- |
| `{resource_path}/create` | POST | SQL |
| `{resource_path}/get` | GET | QueryBuilder |
| `{resource_path}/update` | PUT | SQL |
| `{resource_path}/delete` | DELETE | SQL |
| `{resource_path}/qb-list` | GET | QueryBuilder |
| `{resource_path}/table` | GET | QueryBuilder |
| `{resource_path}/view-sql-list` | GET | View SQL |

Draft a SQL API when you need a single hand-written query or View SQL endpoint:

```bash
cargo run -- bundle draft-sql \
  --datasource-id postgres_demo \
  --resource-path demo/items/custom-search \
  --api-id demo_items_custom_search \
  --api-name "Demo Items Custom Search" \
  --group-id demo_items_group \
  --group-name "PG Demo Items" \
  --sql 'select id, name from demo_items where status = $status' \
  --engine sql \
  --out target/dbapi-bundles/demo_items_custom_search
```

SQL value parameters use named placeholders such as `$status`; positional placeholders such as `$1` are rejected. Use engine `sql` for plain SQL APIs or `viewSql` for View SQL APIs.

## MCP Sidecar

Docker Compose starts an MCP HTTP sidecar at `http://127.0.0.1:8521/mcp`:

```bash
docker compose up -d --build
```

For local binary debugging, if you want demo datasources (`local_sqlite_demo`, `mysql_demo`, `postgres_demo`) returned by `list_datasources`, start `serve` with the seeded DB explicitly:

```bash
DB_API_METADATA_URL=sqlite://$(pwd)/data.db /Users/andy/Target/debug/db-api-rs serve
```

Then in another terminal:

```bash
/Users/andy/Target/debug/db-api-rs mcp --transport http --listen 127.0.0.1:8521 --base-url http://127.0.0.1:8520
```

Desktop MCP clients can also launch stdio transport:

```bash
cargo run -- mcp --transport stdio --base-url http://127.0.0.1:8520
```

The sidecar defaults to read/draft/validate mode. Write-capable tools require both starting the process with `--allow-write` and passing tool request `allowWrite=true`. `call_published_api` can run GET smoke tests without write access; non-GET calls require the write gates.

Inspect the public MCP surface without starting a client:

```bash
cargo run -- mcp inspect --json
```

Call a tool locally through the same DBAPI client path used by the MCP sidecar:

```bash
cargo run -- mcp --base-url http://127.0.0.1:8520 call health_check
cargo run -- mcp --base-url http://127.0.0.1:8520 call list_tables --args-json '{"datasourceId":"postgres_demo"}'
```

Run a QA smoke check:

```bash
cargo run -- qa smoke --base-url http://127.0.0.1:8520
cargo run -- qa smoke --base-url http://127.0.0.1:8520 --path demo/items/qb-list --params-json '{"limit":20,"offset":0}'
```

Available tools:

- `health_check`
- `list_datasources`
- `list_groups`
- `list_api_configs`
- `list_tables`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `create_app_token_for_group`
- `call_published_api`
- `validate_api_bundle`
- `apply_api_config_bundle`

Available resources:

- `dbapi://docs/quickstart`
- `dbapi://docs/bundle-workflow`
- `dbapi://api-catalog`
- `dbapi://datasources`
- `dbapi://skills`

Available prompts:

- `generate_table_api_bundle`
- `generate_sql_api_bundle`
- `review_bundle_before_apply`
- `qa_smoke_test_plan`

The sidecar calls the existing HTTP management routes on the main DBAPI service. It does not directly read or write `data.db`.

Example MCP client config:

```json
{
  "mcpServers": {
    "db-api-rs": {
      "url": "http://127.0.0.1:8521/mcp"
    }
  }
}
```

The repo-local skill catalog is available at `skills/index.json`. Skills are workflow documentation, not a separate runtime; the CLI and MCP server are the executable surfaces.

## Repository

Canonical GitHub repository:

```text
https://github.com/thomas7725353/db-api-rs
```
