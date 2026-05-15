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

Desktop MCP clients can also launch stdio transport:

```bash
cargo run -- mcp --transport stdio --base-url http://127.0.0.1:8520
```

Inspect the MCP surface from the CLI:

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

MCP tools:

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

MCP resources:

- `dbapi://docs/quickstart`
- `dbapi://docs/bundle-workflow`
- `dbapi://api-catalog`
- `dbapi://datasources`
- `dbapi://skills`

MCP prompts:

- `generate_table_api_bundle`
- `generate_sql_api_bundle`
- `review_bundle_before_apply`
- `qa_smoke_test_plan`

By default, the sidecar only supports read, draft, and validation workflows. Write-capable tools require starting the service with `--allow-write` and passing `allowWrite=true`. `call_published_api` can run GET smoke tests without write access; non-GET calls require the write gates.

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

## ASCII Architecture

```text
=====================================================================
                Bundle Workflow (File-first + Review Gate)
=====================================================================

                     +-----------------------------+
                     |            Agent/CLI         |
                     |  - agent prompt              |
                     |  - skill execution           |
                     +--------------+--------------+
                                    |
                                    | 1. Generate bundle files
                                    v
                 +------------------+------------------+
                 |            db-api-rs CLI            |
                 | bundle draft-table / draft-sql       |
                 +------------------+------------------+
                                    |
                                    | 2. Produce review artifacts
                                    v
                 +---------------------------------------+
                 | Target bundle directory                |
                 |  dbapi_manifest.json                  |
                 |  api_group_config.json                |
                 |  api_config.json                      |
                 |  curl.md                              |
                 |  VERIFY.md                            |
                 +------------------+--------------------+
                                    |
                                    | 3. Human review
                                    v
                 +---------------------+-------------------+
                 | db-api-rs CLI (or MCP tool) validate     |
                 |  cargo run -- bundle validate            |
                 |  OR                                     |
                 |  mcp call validate_api_bundle             |
                 +---------------------+-------------------+
                                    |
                                    | 4. Write gate
                                    |    --allow-write
                                    v
                 +------------------------+---------------+
                 |        db-api-rs CLI/MCP apply         |
                 | apply_api_config_bundle                |
                 +------------------------+---------------+
                                          |
                                          | 5. Write to runtime metadata DB
                                          v
                              +----------------------------+
                              |    db-api-rs Runtime 8520   |
                              |    /apiConfig/importGroup   |
                              |    /apiConfig/import        |
                              +-------------+--------------+
                                            |
                                            v
                              +----------------------------+
                              |      SQLite metadata DB      |
                              |         (data.db)           |
                              +-------------+--------------+
                                            |
                                            | 6. Expose APIs
                                            v
                               +---------------------------+
                               |  /api/{path} endpoints    |
                               +-------------+-------------+
                                             |
                                             v
                               +---------------------------+
                               | Business data sources      |
                               | sqlite / mysql / postgres  |
                               +---------------------------+
```

```text
========================================================
                    MCP + Agent Runtime Flow
========================================================

 +--------------------------+           HTTP/stdio           +--------------------------+
 |        Agent / Cursor    | <----------------------------> |   db-api-rs MCP Server    |
 |  - mcp client / tools    |      /mcp                      |   (8521)                 |
 |  - no UI required        |                               |                          |
 +------------+-------------+                               +------------+-------------+
              |                                                           |
              | call tool: health_check/list_*                             |
              +----------------------------------------------------------->|
              |                                                           |
              |                     MCP response                            |
              +<-----------------------------------------------------------+
              |                                                           |
              | call tool: draft/validate/apply/get call_published_api     |
              +----------------------------------------------------------->|
              |                                                           |
              v                                                           v
   +----------------------+                                   +----------------------+
   | CLI-equivalent APIs |                                   | Internal DBAPI Client |
   | (same runtime path) |                                   |  against :8520       |
   +---------+------------+                                   +----------+-----------+
             |                                                           |
             +------------------------------+----------------------------+
                                            |
                                            v
                                    +-------+-----------------+
                                    |   db-api-rs Runtime 8520 |
                                    |   /health, /apiConfig/*  |
                                    |   /api/{path}            |
                                    +-----------+--------------+
                                                |
                                                v
                                         +------+------+
                                         | Access Log + |
                                         | Metadata DB |
                                         +------+------+
                                                |
                                                v
                                   +------------+------------+
                                   |   Target datasource(s)   |
                                   |  sqlite/mysql/postgres   |
                                   +-------------------------+
```

```text
=====================================================
              Agent-First Testing (No UI) Pipeline
=====================================================

Agent Test Runner
        |
        | 1) inspect tools + resources + prompts
        v
db-api-rs mcp inspect --json
        |
        | 2) health + discovery
        v
health_check -> list_datasources -> list_groups -> list_api_configs
        |
        | 3) read checks
        v
list_tables / inspect_table_schema
        |
        | 4) smoke check
        v
qa smoke (GET path) / call_published_api (GET)
        |
        | 5) security check
        v
POST without token -> expect 401 + "No Token!"
        |
        | 6) write checks (gated)
        v
MCP --allow-write + allowWrite=true
apply only after explicit approve
        |
        v
Validation passed and endpoints executed by QA
```

## Agent-First Testing (No UI)

Use this section when testing with an agent, MCP client, or CI tasks.

1. Discover the MCP surface:

```bash
db-api-rs mcp inspect --json
```

2. Verify runtime health and core read APIs:

```bash
db-api-rs mcp --base-url http://127.0.0.1:8520 call health_check
db-api-rs mcp --base-url http://127.0.0.1:8520 call list_datasources
db-api-rs mcp --base-url http://127.0.0.1:8520 call list_groups
db-api-rs mcp --base-url http://127.0.0.1:8520 call list_api_configs
```

3. Run endpoint smoke checks with CLI:

```bash
db-api-rs qa smoke --base-url http://127.0.0.1:8520
db-api-rs qa smoke --base-url http://127.0.0.1:8520 --path demo/items/qb-list --params-json '{"limit":20,"offset":0}'
```

4. For private API paths, expect a token error before token exists:

```bash
db-api-rs qa smoke --base-url http://127.0.0.1:8520 --path demo/items/create --method POST --params-json '{"name":"test-item","status":"active","note":"agent test"}'
```

`No Token!` for private paths is a valid negative-security assertion.

5. Write-capable MCP tools and non-GET published API calls require two gates:

- start MCP with `--allow-write`
- pass `allowWrite: true` in the tool arguments

Example:

```bash
db-api-rs mcp --transport http --listen 127.0.0.1:8521 --base-url http://127.0.0.1:8520 --allow-write
```

Then call:

```bash
db-api-rs mcp --base-url http://127.0.0.1:8520 call create_app_token_for_group --args-json '{"allowWrite":true,"appName":"agent-smoke","groupId":"demo_crud_group"}'
db-api-rs mcp --base-url http://127.0.0.1:8520 call call_published_api --args-json '{"allowWrite":true,"method":"POST","path":"demo/items/create","params":{"name":"agent created","status":"active","note":"agent test"}}'
```

Repo-local agent skills are already aligned to this flow:

- `skills/dbapi-generate-table-apis`
- `skills/dbapi-generate-sql-api`
- `skills/dbapi-apply-api-bundle`
- `skills/dbapi-token-workflow`
- `skills/dbapi-export-import-workflow`

Use these skills to generate reviewable bundles, validate, and only apply after confirmation.

## Agent Skills

The repository includes repo-local skills for Codex, Claude, Cursor, and other agents to reuse stable workflows:

- `skills/index.json`
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
