---
name: dbapi-export-import-workflow
description: Use when exporting or importing DBAPI API configs and API groups through the current HTTP management endpoints.
---

# DBAPI Export Import Workflow

## Purpose

Export and import API configs and API groups through the current DBAPI HTTP endpoints. Imports use multipart field `file`.

Whenever DBAPI core API creation, import/export, or token behavior changes, update this repo-local skill in the same change.

## API Config Export

```bash
rtk curl -sS -X POST \
  "http://127.0.0.1:8520/apiConfig/downloadConfig?ids=$API_IDS" \
  -o api_config.json
```

`API_IDS` is a comma-separated list of API config IDs.

## API Config Import

```bash
rtk curl -sS -X POST \
  -F "file=@api_config.json" \
  "http://127.0.0.1:8520/apiConfig/import"
```

## Group Export

```bash
rtk curl -sS -X POST \
  "http://127.0.0.1:8520/apiConfig/downloadGroupConfig?ids=$GROUP_IDS" \
  -o api_group_config.json
```

`GROUP_IDS` is a comma-separated list of API group IDs.

## Group Import

```bash
rtk curl -sS -X POST \
  -F "file=@api_group_config.json" \
  "http://127.0.0.1:8520/apiConfig/importGroup"
```

For complete bundle application, prefer `dbapi-apply-api-bundle`, which validates before importing.
