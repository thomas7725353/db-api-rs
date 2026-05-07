---
name: dbapi-apply-api-bundle
description: Use when validating or applying generated DBAPI bundle files through the local DBAPI HTTP management API.
---

# DBAPI Apply API Bundle

## Purpose

Validate and apply generated DBAPI bundle files after human review. Apply only after the user confirms the generated `dbapi_manifest.json`, `api_group_config.json`, `api_config.json`, `curl.md`, and `VERIFY.md`.

Whenever DBAPI core API creation, import/export, or token behavior changes, update this repo-local skill in the same change.

## Validate

```bash
rtk cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR"
```

Validation checks local bundle shape, datasource existence, and server group/API conflicts before apply.

## Apply

Use `--allow-write`, not `--allow-write=true`.

```bash
rtk cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir "$BUNDLE_DIR" \
  --allow-write
```

After apply, run the generated checks in `VERIFY.md` and the requests in `curl.md`.
