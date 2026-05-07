---
name: dbapi-demo-crud
description: Use when seeding or verifying the local Rust standalone db-api-rs Demo CRUD group, demo_items SQLite table, private CRUD/qb-list APIs, app authorization, token generation, authenticated curl calls, and access-log monitor data.
---

# db-api-rs Demo CRUD

## Purpose

Use this skill only for the historical local demo CRUD seed. For new API creation, use `dbapi-generate-table-apis`, `dbapi-generate-sql-api`, and `dbapi-apply-api-bundle`.

Use this skill to recreate the known-good demo API set for the Rust standalone db-api-rs runtime. The demo uses the repository root `data.db` as both metadata database and example business SQLite database. The seeded APIs are private (`previlege=0`), so curl verification must create an app, authorize it to `demo_crud_group`, generate a token, then pass `Authorization: $TOKEN`.

## Files

- Seed SQL: `skills/dbapi-demo-crud/scripts/seed_demo_api.sql`
- Runtime seed copy: `seed_demo_api.sql`
- Metadata DB: `data.db`
- Rust runtime: repository root

## Workflow

1. Confirm the current working directory is the repository root.
2. Apply the seed SQL:

```bash
rtk sqlite3 data.db < skills/dbapi-demo-crud/scripts/seed_demo_api.sql
```

3. If `db-api-rs` is already running, restart it so the config cache is clean. The service normally listens on `127.0.0.1:8520`.
4. Verify the seed result:

```bash
rtk sqlite3 data.db "select id,name from api_group where id='demo_crud_group'; select id,path,status from api_config where group_id='demo_crud_group' order by path;"
```

Expected API paths:

- `demo/items/create`
- `demo/items/get`
- `demo/items/update`
- `demo/items/delete`
- `/demo/items/qb-list`

Do not create separate `filter` or `count` APIs. The list API is the single list endpoint and must support filter, pagination, and total.

## Token Setup

Run this before calling any demo API:

```bash
APP_JSON=$(rtk curl -sS -X POST \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'name=demo-crud-client' \
  --data-urlencode 'note=Demo CRUD token client' \
  --data-urlencode 'expireDesc=forever' \
  'http://127.0.0.1:8520/app/create')

APP_ID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "$APP_JSON")
SECRET=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["secret"])' "$APP_JSON")

rtk curl -sS -X POST \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode "appId=$APP_ID" \
  --data-urlencode 'groupIds=demo_crud_group' \
  'http://127.0.0.1:8520/app/auth/'

TOKEN_JSON=$(rtk curl -sS "http://127.0.0.1:8520/token/generate?appid=$APP_ID&secret=$SECRET")
TOKEN=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["token"])' "$TOKEN_JSON")

printf 'APP_ID=%s\nTOKEN=%s\n' "$APP_ID" "$TOKEN"
```

Expected: `/token/generate` returns JSON containing `token`, `appId`, and `expireAt`. A missing token request to the demo APIs should return HTTP `401` with `msg=No Token!`.

## API Contract

### Create

```bash
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'name=Alpha' \
  --data-urlencode 'status=active' \
  --data-urlencode 'note=first item' \
  'http://127.0.0.1:8520/api/demo/items/create'
```

Expected: `success=true`, `data.rowsAffected=1`.

### Get

```bash
rtk curl -sS \
  -H "Authorization: $TOKEN" \
  'http://127.0.0.1:8520/api/demo/items/get?id=1'
```

Expected: `success=true`, `data` is one object when found, or `null` when not found.

### Update

```bash
rtk curl -sS -X PATCH \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'id=1' \
  --data-urlencode 'name=Alpha2' \
  --data-urlencode 'status=active' \
  --data-urlencode 'note=updated item' \
  'http://127.0.0.1:8520/api/demo/items/update'
```

Expected: `success=true`, `data.rowsAffected` is `0` or `1` depending on whether the row exists.

### Delete

```bash
rtk curl -sS -X DELETE \
  -H "Authorization: $TOKEN" \
  'http://127.0.0.1:8520/api/demo/items/delete?id=2'
```

Expected: `success=true`, `data.rowsAffected` is `0` or `1`.

### QueryBuilder List With Filter, Limit, Offset, And Total

```bash
rtk curl -sS \
  -H "Authorization: $TOKEN" \
  'http://127.0.0.1:8520/api/demo/items/qb-list?keyword=&status=&limit=10&offset=0'
```

Filtered example:

```bash
rtk curl -sS \
  -H "Authorization: $TOKEN" \
  'http://127.0.0.1:8520/api/demo/items/qb-list?keyword=Alpha&status=active&limit=10&offset=0'
```

Expected: `success=true`; each returned row includes `total`. Empty result sets return `data=[]`, so consumers that need total for empty pages must query offset `0` or handle empty totals.

## Verification Checklist

Run these after seeding or editing the runtime:

```bash
rtk cargo test
rtk cargo check
```

Use the repository root for Rust commands.

Then verify with curl:

```bash
rtk curl -sS \
  -H "Authorization: $TOKEN" \
  'http://127.0.0.1:8520/api/demo/items/qb-list?keyword=&status=&limit=10&offset=0'
```

The response must be JSON with `success:true`.

Verify token rejection:

```bash
rtk curl -sS -w '\nHTTP_STATUS=%{http_code}\n' \
  'http://127.0.0.1:8520/api/demo/items/qb-list?keyword=&status=&limit=10&offset=0'
```

Expected: HTTP `401` and JSON `success=false`, `msg=No Token!`.

Verify monitor/access-log data:

```bash
NOW=$(date +%s)
START=$((NOW-604800))

rtk curl -sS -X POST \
  --data-urlencode "start=$START" \
  --data-urlencode "end=$((NOW+60))" \
  'http://127.0.0.1:8520/access/top5api'

rtk curl -sS -X POST \
  --data-urlencode "start=$START" \
  --data-urlencode "end=$((NOW+60))" \
  'http://127.0.0.1:8520/access/search'
```

Expected: `/access/top5api` includes `/api/demo/items/qb-list`, and `/access/search` includes both `401` no-token rows and `200` token-authenticated rows after the verification calls above.
