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

MCP-capable agents can connect to the sidecar on `127.0.0.1:8521`.

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
docs/superpowers/       Planning documents used during development
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
  --datasource postgres_demo \
  --table demo_items \
  --resource-path pg/demo/items \
  --group-id pg_demo_items \
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
  --base-url http://127.0.0.1:8520 \
  --datasource postgres_demo \
  --path pg/demo/items/by-status \
  --group-id pg_demo_items \
  --name "PG Demo Items By Status" \
  --method GET \
  --engine sql \
  --sql "select id, name, status from demo_items where status = $status order by id" \
  --out bundles/pg-demo-items-by-status
```

SQL value parameters use named placeholders such as `$status`; positional placeholders such as `$1` are rejected. Use engine `sql` for plain SQL APIs or `viewSql` for View SQL APIs.

## MCP Sidecar

Docker Compose starts an MCP HTTP sidecar on `127.0.0.1:8521`:

```bash
docker compose up -d --build
```

The sidecar defaults to read/draft/validate mode. Writes require both starting the process with `--allow-write` and passing tool request `allowWrite=true`.

Available tools:

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `apply_api_config_bundle`

The sidecar calls the existing HTTP management routes on the main DBAPI service. It does not directly read or write `data.db`.

## Repository

Canonical GitHub repository:

```text
https://github.com/thomas7725353/db-api-rs
```
