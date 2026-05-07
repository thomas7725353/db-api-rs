# DBAPI Skills and MCP Sidecar Design

## Goal

Make DBAPI API creation fast and repeatable for AI agents without forcing users to manually create groups, APIs, imports, exports, apps, authorization, tokens, and curl examples in the web UI.

The first version uses a file-first workflow with dry-run validation and optional apply. It also defines an MCP HTTP sidecar so Cursor, Codex, Claude, and local agents can call the same manifest, validation, and apply capabilities.

## Current Product Context

DBAPI currently supports these core capabilities:

- HTTP API publishing under `/api/{path}`.
- API creation through three engines:
  - `SQL`
  - `QueryBuilder`
  - `View SQL`
- Business datasources for SQLite and PostgreSQL, with MySQL planned as a supported business datasource.
- API group creation, API group import, API group export, API config import, API config export, and generated API docs.
- App creation, group authorization, token generation, and authenticated API requests.
- Runtime compatibility for API paths stored with or without a leading `/`.

The existing `skills/dbapi-demo-crud` skill is outdated because it only documents the old demo CRUD flow. Skills must now be treated as part of the product surface. Whenever a core DBAPI feature changes, the related skills must be updated in the same phase.

## Scope

- Define a DBAPI manifest workflow for AI-generated API groups and APIs.
- Add or refresh repo-local skills for common API creation and verification workflows.
- Support table-driven API generation.
- Support SQL or natural-language requirement driven API generation.
- Support group import/export, API import/export, app creation, group authorization, token generation, and curl verification in skills.
- Define MCP sidecar deployment using Docker Compose over HTTP.
- Keep generated files reviewable before apply.

## Implementation Sequence

1. DBAPI Manifest v1: define the AI-generatable API bundle format for groups, APIs, SQL rows, params, response modes, curl examples, and validation metadata.
2. Repo-local skills: add or refresh skills for table-to-CRUD/list/view generation, SQL-or-requirement-to-API generation, API bundle apply, token workflow, and import/export workflow.
3. CLI/import validation loop: generate manifest files first, run dry-run validation, then apply through existing DBAPI import/group/app/token routes after explicit confirmation.
4. MCP server: expose the same manifest, validate, apply, export, schema-inspection, and token workflow through an HTTP sidecar for Cursor, Codex, Claude, and local agents.

## Non-Goals

- Do not replace the web UI.
- Do not make MCP the only way to manage APIs.
- Do not put MCP directly into the primary `/api/{path}` business API path.
- Do not let MCP bypass DBAPI's existing HTTP management routes and validation.
- Do not store database passwords or generated tokens in committed manifests.
- Do not require automatic apply for generated APIs.

## Recommended Workflow

Use the combined mode selected during design:

1. Generate reviewable files:
   - `api_group_config.json`
   - `api_config.json`
   - `curl.md`
   - `VERIFY.md`
2. Run dry-run validation:
   - datasource exists
   - table exists
   - selected columns exist
   - primary key is valid when required
   - API paths do not conflict
   - group IDs and names do not conflict
   - QueryBuilder DSL parses
   - View SQL list and count templates render with sample parameters
   - SQL APIs parse and use bind parameters
   - token/app/group authorization plan is valid
3. Apply only after explicit confirmation:
   - import groups
   - import API configs
   - create app
   - authorize app to group
   - generate token
   - run curl verification

## Manifest Rules

Generated API paths must be based on explicit `resource_path`, not guessed from table names.

Example input:

```json
{
  "datasource_id": "postgres_demo",
  "table": "demo_items",
  "primary_key": "id",
  "resource_path": "demo/items",
  "group": {
    "id": "demo_items_group",
    "name": "Demo Items"
  }
}
```

Rules:

- `table` means the real database table.
- `resource_path` means the external DBAPI API path prefix.
- `resource_path` is required.
- Generated manifest paths use no leading `/`.
- Runtime and curl examples show calls under `/api/{path}`.
- If a primary key cannot be discovered, the generator must require `primary_key`.
- If the table has no primary key, skip `get`, `update`, and `delete`.

## Table Generation Design

`draft_table_crud_bundle` is the most important MCP/skill workflow.

It generates these APIs by default:

| API | Method | Path | Engine | Purpose |
| --- | --- | --- | --- | --- |
| create | POST | `{resource_path}/create` | SQL | Insert a row |
| get | GET | `{resource_path}/get` | SQL | Read one row by primary key |
| update | PATCH | `{resource_path}/update` | SQL | Update one row by primary key |
| delete | DELETE | `{resource_path}/delete` | SQL | Delete one row by primary key |
| qb-list | GET | `{resource_path}/qb-list` | QueryBuilder | Page/list API |
| table | GET | `{resource_path}/table` | QueryBuilder | Frontend universal table API |
| view-sql-list | GET | `{resource_path}/view-sql-list` | View SQL | View/report/analysis API |

Engine policy:

- `create`, `get`, `update`, and `delete` use SQL because they are direct primary-key operations and write operations.
- `qb-list` and `table` use QueryBuilder because they are editable, schema-aware list APIs and fit frontend table usage.
- `view-sql-list` uses View SQL because reports, joins, dynamic selected columns, dynamic sorting, and analysis queries need templated SQL.
- Plain SQL remains supported for custom API generation, but table-list generation should prefer QueryBuilder and View SQL.

## QueryBuilder Generation

The generator should create QueryBuilder DSL for:

- selected columns
- default order by primary key descending when available
- `limit` and `offset`
- `count: true` for page responses
- simple filters from likely searchable columns
- exact filters from enum-like or status-like columns

`qb-list` should be a compact list API.

`table` should be a fuller API for frontend universal tables:

- broader column selection
- stable pagination
- stable ordering
- filter definitions that can be mapped to table search controls

Both APIs use `resultType=page`.

## View SQL Generation

The generator should create:

- one `viewSql` SQL template
- one `viewSqlCount` template when response mode is `page` or `count`
- parameter definitions for normal bind parameters
- sample preview parameters for structure fragments

Default View SQL template shape:

```sql
select [[ columns | ident_list ]]
from table_name a
where 1 = 1
order by [[ order_by | ident ]] desc
limit [[ limit | int(default=20,max=100) ]]
offset [[ offset | int(default=0) ]]
```

Normal values must use `$param` bind parameters. Structure fragments must use the existing View SQL safe filters such as `ident`, `ident_list`, and `int(...)`.

## SQL Generation

SQL generation covers direct row operations:

- `insert into table (...) values ($...)`
- `select ... from table where pk = $pk`
- `update table set ... where pk = $pk`
- `delete from table where pk = $pk`

Rules:

- Use bind parameters for all values.
- Do not include generated/default timestamp columns in request params unless explicitly requested.
- Do not update the primary key.
- Use `application/x-www-form-urlencoded` for generated direct CRUD APIs unless the user requests JSON.
- Generated `get` APIs should return object mode.

## SQL or Requirement Driven API Design

`draft_sql_api_bundle` supports:

- a user-provided SQL query
- a user-provided SQL write statement
- a natural-language requirement that the agent turns into SQL or View SQL

Rules:

- Query/report requirements should prefer View SQL when dynamic columns, ordering, joins, or analysis are involved.
- Simple one-off SQL can use SQL engine.
- The generated bundle must include params, method, response mode, curl examples, and validation notes.
- The skill must ask for missing datasource, resource path, group, table, or primary key information instead of guessing risky values.

## Skills

First version skills:

- `dbapi-generate-table-apis`: generate CRUD, QueryBuilder list/table, View SQL report APIs from datasource, table, primary key, and resource path.
- `dbapi-generate-sql-api`: generate an API from SQL or a natural-language requirement.
- `dbapi-apply-api-bundle`: validate and apply generated group/API files through DBAPI import routes.
- `dbapi-token-workflow`: create app, authorize groups, generate token, and produce curl verification.
- `dbapi-export-import-workflow`: export/import groups and APIs with verification.

Skill maintenance rule:

- Any phase that changes API config shape, engine behavior, route names, import/export format, datasource support, app authorization, token behavior, or curl/request semantics must update the affected DBAPI skills before the phase is considered complete.
- Verification for such phases must include reading the affected `skills/*/SKILL.md` files and checking that examples match current routes, engines, params, and expected responses.

## MCP Sidecar Design

MCP is a second entrance for agents, not a replacement for skills or the web UI.

Recommended deployment:

```yaml
services:
  db-api-rs:
    image: db-api-rs:latest
    ports:
      - "127.0.0.1:8520:8520"

  dbapi-mcp:
    image: db-api-rs:latest
    command:
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
      - db-api-rs
```

MCP sidecar rules:

- It calls DBAPI's existing HTTP management routes.
- It does not directly read or write `data.db`.
- It defaults to read/draft/validate/export mode.
- Write tools require both process-level write enablement and tool-level confirmation.
- It binds to localhost by default.
- It should be implemented with `rmcp` rather than `rig` for the first version because DBAPI needs an MCP server adapter, not an embedded LLM agent framework.

## MCP Tools

Read/draft tools:

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `export_api_group`
- `generate_curl_examples`

Write tools:

- `apply_api_group_bundle`
- `apply_api_config_bundle`
- `create_app_token_for_group`

Write tool guard:

```text
process starts with --allow-write=true
and tool call passes allow_write=true
```

If either condition is missing, the tool must return a dry-run result instead of writing.

## Error Handling

- Missing datasource returns a clear validation error.
- Missing table returns a clear validation error.
- Missing primary key blocks get/update/delete generation.
- Duplicate API paths block apply.
- Duplicate group IDs or names block apply.
- Unsupported column types fall back to string params with a warning.
- View SQL preview failures block apply.
- QueryBuilder parse failures block apply.
- SQL parse or execution-preview failures block apply when a preview is requested.
- Token creation is never written to generated manifest files.

## Testing and Verification

Spec-level verification:

- Generated table bundle includes all seven expected APIs.
- Generated paths use explicit `resource_path`.
- Generated paths do not include a leading `/`.
- `create/get/update/delete` use SQL.
- `qb-list/table` use QueryBuilder.
- `view-sql-list` uses View SQL and includes count SQL for page mode.
- Dry-run catches missing datasource, missing table, missing primary key, duplicate path, and invalid engine payloads.
- Apply uses existing DBAPI import/group/app/token routes.
- Skills examples match the current DBAPI routes and engine names.

Runtime verification:

- Start DBAPI and MCP sidecar with Docker Compose.
- Call MCP `list_datasources`.
- Call MCP `inspect_table_schema`.
- Call MCP `draft_table_crud_bundle`.
- Validate the generated files.
- Apply with write disabled and confirm no writes occur.
- Enable writes explicitly and apply to a local test group.
- Generate token and run curl examples.

## Open Implementation Notes

- Schema introspection should be extended to expose primary key, nullable, default, and generated/autoincrement metadata. Until then, `primary_key` is required for safe CRUD generation.
- MySQL generation should use the same manifest shape. Dialect differences belong in SQL/QueryBuilder rendering, not in the external skill workflow.
- Existing path compatibility allows leading or non-leading slashes, but generated files should standardize on no leading slash.
