# Agent and MCP Workflow

DBAPI includes repo-local skills and an MCP sidecar so AI coding agents can generate APIs through the same GitOps workflow as humans.

## Agent Rule

Agents must not write directly to DBAPI metadata as their first step. They should generate reviewable bundle files, validate them, and apply only after human confirmation.

## Repo-Local Skills

Available skills:

- `skills/dbapi-generate-table-apis`
- `skills/dbapi-generate-sql-api`
- `skills/dbapi-apply-api-bundle`
- `skills/dbapi-token-workflow`
- `skills/dbapi-export-import-workflow`

Use table generation when the request is table-backed:

```text
use skill dbapi-generate-table-apis 给 postgres_demo.demo_items 生成 API，
resource_path=ops/postgres/items，group_id=ops_pg_items_group，group_name=Ops PG Items
```

Use SQL generation when the requirement is already expressed as SQL:

```text
use skill dbapi-generate-sql-api 生成一个 viewSql 报表 API，
datasource_id=postgres_demo，resource_path=ops/report/items，
api_id=ops_report_items，group_id=ops_report_group
```

Use apply only after review:

```text
use skill dbapi-apply-api-bundle validate target/dbapi-bundles/ops_pg_items_group，
确认后再 apply --allow-write
```

## MCP Sidecar

Docker Compose starts an MCP HTTP sidecar:

```text
http://127.0.0.1:8521/mcp
```

Available tools:

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `apply_api_config_bundle`

The sidecar calls DBAPI HTTP management routes. It does not directly read or write `data.db`.

## Write Safety

The MCP sidecar defaults to read, draft, and validate mode.

Writes require both:

- Starting the sidecar with `--allow-write`.
- Passing tool request `allowWrite=true`.

This double gate is intentional and should remain in company deployments.

## Required Inputs For Agents

When asking an agent to generate APIs, provide:

- `base_url`
- `datasource_id`
- `table` or SQL
- explicit `resource_path`
- `group_id`
- `group_name`
- `api_id` and `api_name` for SQL APIs
- intended engine when using SQL
- primary key only when inference is not enough

Do not ask agents to infer `resource_path` from table names.

## Agent Output Review

Before accepting agent-generated bundles, review:

- Whether paths match company naming rules.
- Whether method and engine choices are correct.
- Whether SQL uses named parameters.
- Whether generated curl commands are safe.
- Whether `VERIFY.md` is executable in the target environment.

If the agent changes DBAPI creation, import/export, token, or apply behavior, it must update the repo-local skills in the same change.
