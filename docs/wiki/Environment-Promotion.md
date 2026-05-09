# Environment Promotion

Promotion means applying the same reviewed API bundle through dev, staging, and production with controlled environment differences.

## Environments

Recommended lifecycle:

```text
local
  -> dev
  -> staging
  -> production
```

Local is for exploration. Dev, staging, and production require reviewed bundle files.

## Promotion Artifacts

Promote these files together:

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`
- environment patch notes when datasource IDs differ

Do not promote an API by manually recreating it in the UI.

## Environment Differences

Allowed environment differences:

- `base_url`
- datasource ID
- token value
- safe test request values in `curl.md`

Not allowed without a new review:

- API path changes.
- HTTP method changes.
- SQL logic changes.
- response mode changes.
- token/public mode changes.
- engine changes.

## Promotion Steps

For each environment:

1. Confirm target DBAPI server health.
2. Confirm datasource exists.
3. Apply reviewed environment patch if needed.
4. Run `bundle validate`.
5. Apply with `--allow-write`.
6. Run `curl.md`.
7. Complete `VERIFY.md`.
8. Check access logs.
9. Record promotion evidence.

## Export And Import

For existing APIs, export current configuration before migration:

```bash
rtk curl -sS -X POST \
  "http://127.0.0.1:8520/apiConfig/downloadConfig?ids=$API_IDS" \
  -o api_config.json
```

Export groups:

```bash
rtk curl -sS -X POST \
  "http://127.0.0.1:8520/apiConfig/downloadGroupConfig?ids=$GROUP_IDS" \
  -o api_group_config.json
```

Import API config:

```bash
rtk curl -sS -X POST \
  -F "file=@api_config.json" \
  "http://127.0.0.1:8520/apiConfig/import"
```

Import groups:

```bash
rtk curl -sS -X POST \
  -F "file=@api_group_config.json" \
  "http://127.0.0.1:8520/apiConfig/importGroup"
```

For new GitOps-managed APIs, prefer full bundle apply because it validates before import.

## Rollback

Rollback options:

| Situation | Action |
| --- | --- |
| Bad new API path | Disable or remove the new API config, then apply the previous reviewed bundle |
| Breaking change to existing path | Restore previous reviewed `api_config.json` |
| Bad datasource credentials | restore previous datasource config or rotate credentials |
| Bad SQL behavior | revert the bundle change in Git and reapply the previous bundle |

Rollback must leave an audit trail with the previous bundle path and verification evidence.
