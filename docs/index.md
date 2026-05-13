# db-api-rs Documentation

`db-api-rs` is a standalone Rust runtime with a React admin UI for publishing database queries as HTTP APIs. It is designed for internal tools, admin systems, data service prototypes, and workflows that need to expose SQLite, MySQL, or PostgreSQL tables and SQL queries as HTTP endpoints.

## Quick Start

Start the service with Docker Compose:

```bash
docker compose up -d --build
```

Default service URL:

```text
http://127.0.0.1:8520
```

Health check:

```bash
curl http://127.0.0.1:8520/health
```

Compose mounts the repository `data.db` file into the container as the metadata database and local demo database:

```yaml
volumes:
  - ./data.db:/data/data.db
```

## Core Features

- Manage data sources in the web UI.
- Publish APIs under `/api/{path}`.
- Support public APIs and token-protected APIs.
- Record API access logs.
- Support SQLite, MySQL, and PostgreSQL data sources.
- Configure HTTP methods including `GET`, `POST`, `PUT`, `PATCH`, and `DELETE`.

## API Creation Modes

### QueryBuilder

QueryBuilder uses a structured query DSL to create common list, filter, pagination, and count APIs. It is a good fit for table-level CRUD, list pages, and admin queries with clear rules.

### SQL

SQL mode is for fixed hand-written SQL. Parameters use named placeholders such as `$name`:

```sql
select id, name from demo_items where status = $status
```

Do not use positional placeholders such as `$1`.

### View SQL

View SQL uses MiniJinja templates to generate safe SQL structure fragments. It is useful for complex joins, dynamic columns, sort fields, limits, and offsets.

Example:

```sql
select [[ columns | ident_list ]]
from demo_items
where status = $status
order by [[ order_by | ident ]] desc
limit [[ limit | int(default=10,max=1000) ]]
offset [[ offset | int(default=0) ]]
```

Value parameters are still bound with `$status`; structure parameters can only be rendered through restricted safety filters.

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

## Bundle Workflow

Use the bundle workflow to generate, review, validate, and import API configuration in batches. A generated bundle directory usually contains:

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`

Generate a CRUD/list/view bundle from a table:

```bash
cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id postgres_demo \
  --table demo_items \
  --resource-path pg/demo/items \
  --group-id demo_items_group \
  --group-name "PG Demo Items" \
  --out target/dbapi-bundles/demo_items
```

Validate the bundle:

```bash
cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items
```

After reviewing the files, import the bundle:

```bash
cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items \
  --allow-write
```

Generate a single API bundle from SQL:

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

Supported `--engine` values:

- `sql`
- `viewSql`

## MCP Sidecar

Docker Compose also starts an MCP HTTP sidecar:

```text
http://127.0.0.1:8521/mcp
```

MCP tools:

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `apply_api_config_bundle`

By default, the sidecar only supports read, draft, and validation workflows. Writes require starting the service with `--allow-write` and passing `allowWrite=true` when calling the apply tool.

## Agent Skills

The repository includes repo-local skills for Codex, Claude, Cursor, and other agents to reuse stable workflows:

- `skills/dbapi-generate-table-apis`
- `skills/dbapi-generate-sql-api`
- `skills/dbapi-apply-api-bundle`
- `skills/dbapi-token-workflow`
- `skills/dbapi-export-import-workflow`

Example prompt:

```text
use skill dbapi-generate-table-apis to generate APIs for postgres_demo.demo_items,
resource_path=demo/items, group_id=demo_items_group, group_name=PG Demo Items
```

## Project URL

GitHub repository:

```text
https://github.com/thomas7725353/db-api-rs
```
