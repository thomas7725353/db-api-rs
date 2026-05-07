---
name: dbapi-generate-sql-api
description: Use when generating one reviewable DBAPI API bundle from SQL or from a requirement already converted into SQL or View SQL.
---

# DBAPI Generate SQL API

## Purpose

Generate one reviewable API bundle from SQL, or from a requirement that has already been converted into SQL or View SQL. Do not write directly to DBAPI metadata from this skill; generate files, review them, then use `dbapi-apply-api-bundle`.

Whenever DBAPI core API creation, import/export, or token behavior changes, update this repo-local skill in the same change.

## Rules

- Use named params such as `$status`.
- Do not use positional placeholders such as `$1`; they are rejected.
- `--engine` must be `sql` or `viewSql`; invalid engines are rejected.
- Prefer `viewSql` for dynamic, reporting, and analysis APIs.
- `draft-sql` does not require `--base-url`.
- `resource_path` must be explicit; do not infer it from the API name or SQL.

## Command

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

Use `--engine viewSql` when the SQL should run through the View SQL engine. The bundle directory contains `dbapi_manifest.json`, `api_group_config.json`, `api_config.json`, `curl.md`, and `VERIFY.md`.
