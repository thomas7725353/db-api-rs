---
name: dbapi-demo-crud
description: Use when seeding or verifying the local Rust standalone db-api Demo CRUD group, demo_items SQLite table, private CRUD/list APIs, app authorization, token generation, authenticated curl calls, and access-log monitor data.
---

# DBApi Demo CRUD

## Purpose

Use this skill to recreate the known-good demo API set for the Rust standalone db-api runtime. The demo uses the repository root `data.db` as both metadata database and example business SQLite database. The seeded APIs are private (`previlege=0`), so curl verification must create an app, authorize it to `demo_crud_group`, generate a token, then pass `Authorization: $TOKEN`.

## Files

- Seed SQL: `skills/dbapi-demo-crud/scripts/seed_demo_api.sql`
- Runtime seed copy: `runtime-rust/seed_demo_api.sql`
- Metadata DB: `data.db`
- Rust runtime: `runtime-rust`

## Workflow

1. Confirm the current working directory is `/Users/andy/RustroverProjects/db-api`.
2. Apply the seed SQL:

```bash
rtk sqlite3 data.db < skills/dbapi-demo-crud/scripts/seed_demo_api.sql
```

3. If `runtime-rust` is already running, restart it so the config cache is clean. The service normally listens on `127.0.0.1:8520`.
4. Verify the seed result:

```bash
rtk sqlite3 data.db "select id,name from api_group where id='demo_crud_group'; select id,path,status from api_config where group_id='demo_crud_group' order by path;"
```

Expected API paths:

- `demo/items/create`
- `demo/items/get`
- `demo/items/update`
- `demo/items/delete`
- `demo/items/list`

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
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'id=1' \
  'http://127.0.0.1:8520/api/demo/items/get'
```

Expected: `success=true`, `data` is one object when found, or `null` when not found.

### Update

```bash
rtk curl -sS -X POST \
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
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'id=2' \
  'http://127.0.0.1:8520/api/demo/items/delete'
```

Expected: `success=true`, `data.rowsAffected` is `0` or `1`.

### List With Filter, Limit, Offset, And Total

```bash
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'keyword=' \
  --data-urlencode 'status=' \
  --data-urlencode 'limit=10' \
  --data-urlencode 'offset=0' \
  'http://127.0.0.1:8520/api/demo/items/list'
```

Filtered example:

```bash
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'keyword=Alpha' \
  --data-urlencode 'status=active' \
  --data-urlencode 'limit=10' \
  --data-urlencode 'offset=0' \
  'http://127.0.0.1:8520/api/demo/items/list'
```

Expected: `success=true`; each returned row includes `total`. Empty result sets return `data=[]`, so consumers that need total for empty pages must query offset `0` or handle empty totals.

## Verification Checklist

Run these after seeding or editing the runtime:

```bash
rtk cargo test
rtk cargo check
```

Use `workdir=/Users/andy/RustroverProjects/db-api/runtime-rust` for Rust commands.

Then verify with curl:

```bash
rtk curl -sS -X POST \
  -H "Authorization: $TOKEN" \
  --data-urlencode 'keyword=' \
  --data-urlencode 'status=' \
  --data-urlencode 'limit=10' \
  --data-urlencode 'offset=0' \
  'http://127.0.0.1:8520/api/demo/items/list'
```

The response must be JSON with `success:true`.

Verify token rejection:

```bash
rtk curl -sS -w '\nHTTP_STATUS=%{http_code}\n' -X POST \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'keyword=' \
  --data-urlencode 'status=' \
  --data-urlencode 'limit=10' \
  --data-urlencode 'offset=0' \
  'http://127.0.0.1:8520/api/demo/items/list'
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

Expected: `/access/top5api` includes `/api/demo/items/list`, and `/access/search` includes both `401` no-token rows and `200` token-authenticated rows after the verification calls above.
