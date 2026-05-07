---
name: dbapi-generate-table-apis
description: Use when generating reviewable DBAPI bundle files for create/get/update/delete/qb-list/table/view-sql-list APIs from an existing datasource table.
---

# DBAPI Generate Table APIs

## Purpose

Generate reviewable bundle files from a datasource table. Do not write directly to DBAPI metadata from this skill; generate files, review them, then use `dbapi-apply-api-bundle`.

Whenever DBAPI core API creation, import/export, or token behavior changes, update this repo-local skill in the same change.

## Required Inputs

- `base_url`: DBAPI server URL, for example `http://127.0.0.1:8520`
- `datasource_id`: existing DBAPI datasource ID
- `table`: source table name
- `resource_path`: explicit API path prefix, without `/api`; do not guess this from the table name
- `group_id`: API group ID
- `group_name`: API group display name
- `primary_key`: required when the table generator cannot infer the intended key

## Engine Rules

- `create`, `get`, `update`, and `delete` use SQL.
- `qb-list` and `table` use QueryBuilder.
- `view-sql-list` uses View SQL plus View SQL Count.
- Generated paths are `create`, `get`, `update`, `delete`, `qb-list`, `table`, and `view-sql-list` under `resource_path`.

## Command

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

Add `--primary-key "$PRIMARY_KEY"` only when table metadata has no primary key or the intended key must be overridden.

The bundle directory contains `dbapi_manifest.json`, `api_group_config.json`, `api_config.json`, `curl.md`, and `VERIFY.md`.

Next, inspect the files and use `dbapi-apply-api-bundle` to validate and apply.
