# GitOps Workflow

The company workflow is file-first. API configuration is generated into reviewable files, validated, applied with explicit write permission, and promoted through environments using the same reviewed artifacts.

## Principles

- Generate before writing.
- Review generated files in Git.
- Validate against the target server before apply.
- Apply only with explicit write permission.
- Verify the published endpoint after apply.
- Promote the same reviewed bundle across environments.

## Standard Table API Flow

Use this flow when exposing a table as CRUD, list, table, and view-list APIs.

```bash
rtk cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id "$DATASOURCE_ID" \
  --table "$TABLE" \
  --resource-path "$RESOURCE_PATH" \
  --group-id "$GROUP_ID" \
  --group-name "$GROUP_NAME" \
  --out "target/dbapi-bundles/$GROUP_ID"
```

Add `--primary-key "$PRIMARY_KEY"` only when the table metadata cannot infer the key or the team intentionally overrides it.

The generated table bundle creates these paths under `resource_path`:

| Path | Method | Engine |
| --- | --- | --- |
| `{resource_path}/create` | POST | SQL |
| `{resource_path}/get` | GET | QueryBuilder |
| `{resource_path}/update` | PUT | SQL |
| `{resource_path}/delete` | DELETE | SQL |
| `{resource_path}/qb-list` | GET | QueryBuilder |
| `{resource_path}/table` | GET | QueryBuilder |
| `{resource_path}/view-sql-list` | GET | View SQL |

## Standard SQL API Flow

Use this flow when exposing a single hand-written SQL or View SQL endpoint.

```bash
rtk cargo run -- bundle draft-sql \
  --datasource-id "$DATASOURCE_ID" \
  --resource-path "$RESOURCE_PATH" \
  --api-id "$API_ID" \
  --api-name "$API_NAME" \
  --group-id "$GROUP_ID" \
  --group-name "$GROUP_NAME" \
  --sql "$SQL" \
  --engine sql \
  --out "target/dbapi-bundles/$API_ID"
```

Use `--engine viewSql` for reporting, analytics, dynamic columns, dynamic order, limit, or offset.

## Review

Every bundle directory must be reviewed before apply:

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`

The reviewer checks path naming, HTTP method, datasource, SQL safety, response mode, token policy, and the generated curl examples.

## Validate

Validate against the target DBAPI service:

```bash
rtk cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR"
```

Validation checks local bundle shape, datasource existence, and server-side group/API conflicts.

## Apply

Apply only after review and validation:

```bash
rtk cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR" \
  --allow-write
```

Use `--allow-write`, not `--allow-write=true`.

## Verify

After apply:

1. Run the commands in `curl.md`.
2. Complete the checks in `VERIFY.md`.
3. Confirm access log entries exist for success and expected failure cases.
4. Record the bundle path, reviewer, environment, and apply time in the change record.

## Promotion Rule

Do not recreate APIs manually in each environment. Promote the same reviewed bundle files from dev to staging to production.

If an environment-specific datasource ID is needed, create a small environment patch and review that patch in Git before validation and apply.
