# API Import/Export and PostgreSQL CRUD Demo Design

## Goal

Restore the old DBAPI API grouping, import, and export workflow in the Rust admin UI before creating any PostgreSQL demo APIs. The runtime continues to use SQLite as the metadata database. PostgreSQL is added only as a business datasource, alongside the existing SQLite and MySQL datasource support.

## Scope

- Keep `DB_API_METADATA_URL` pointed at SQLite metadata storage.
- Add a PostgreSQL service to `docker-compose.yml` as a business datasource for local demos.
- Support API groups as first-class list-page controls:
  - group filter
  - group management
  - group export
  - group import
- Support API config export and import using the old Java/Vue file shapes:
  - API export: `{ "api": [...], "sql": [...] }`
  - group export: `[{ "id": "...", "name": "..." }]`
- Create a `pg crud` group and copy the confirmed demo API template set into that group with a `/pg` path prefix.

## Non-Goals

- Do not migrate metadata from SQLite to PostgreSQL.
- Do not replace or remove the existing SQLite demo APIs.
- Do not use PostgreSQL to store DBAPI configuration tables.
- Do not add a second import/export format in this phase.

## Confirmed API Group Template

The import/export and PostgreSQL copy flow treats the following APIs as the demo template set:

| Name | Path | Mode |
| --- | --- | --- |
| Demo Item View SQL List | `/demo/items/view-sql-list` | `viewSql` |
| Demo Item QueryBuilder List | `/demo/items/qb-list` | `queryBuilder` |
| 查询 Demo Item | `demo/items/get` | `sql` |
| 创建 Demo Item | `demo/items/create` | `sql` |
| 更新 Demo Item | `demo/items/update` | `sql` |
| 删除 Demo Item | `demo/items/delete` | `sql` |

The PostgreSQL copy keeps these APIs as separate API configs, assigns them to the `pg crud` group, points them at the PostgreSQL datasource, and prefixes paths with `/pg`, for example `/pg/demo/items/get`.

## Backend Design

Add repository functions for batch API and group import/export:

- Load selected `api_config` rows by IDs, including child `api_sql` and `api_alarm` rows when present.
- Load selected `api_group` rows by IDs.
- Insert imported groups.
- Insert imported API configs and their child SQL rows.
- Reject imports with duplicate API IDs, duplicate API paths, duplicate group IDs, or duplicate group names.

Add Rust routes matching the old UI surface:

- `POST /apiConfig/downloadConfig?ids=a,b`
- `POST /apiConfig/import`
- `POST /apiConfig/downloadGroupConfig?ids=a,b`
- `POST /apiConfig/importGroup`
- `POST /apiConfig/apiDocs?ids=a,b`

The upload handler should accept the browser's multipart file upload from Ant Design. The response remains a DBAPI-style JSON success or failure envelope. Export responses return downloadable JSON or Markdown with a `Content-Disposition` filename.

## Frontend Design

Update `ApisPage.tsx` so the list page mirrors the old workflow without changing the overall Ant Design layout:

- Load groups alongside APIs.
- Add a group filter next to keyword search.
- Add a group management modal for creating and deleting groups.
- Add API export and import actions.
- Add group export and import actions.
- Add an export selection modal based on the existing `/apiConfig/getApiTree` data.

The API editor already has a group selector and can keep using the existing `/group/getAll` route.

## PostgreSQL Demo Design

Extend `docker-compose.yml` with a PostgreSQL service:

- Use a stable local demo database, user, password, and exposed host port.
- Leave the DBAPI service metadata URL unchanged.
- Ensure the DBAPI container can connect to PostgreSQL through the compose service name.

Seed or create a PostgreSQL business datasource in SQLite metadata:

- type: `postgres`
- URL: compose-internal PostgreSQL host and database
- username/password: demo credentials
- name: PostgreSQL demo datasource

Create the PostgreSQL demo business table and seed row:

- table: `demo_items`
- columns compatible with the existing demo APIs
- timestamp columns using PostgreSQL defaults where possible

Copy the confirmed API template set into `pg crud`:

- new group: `pg crud`
- new datasource: PostgreSQL demo datasource
- new paths: `/pg/...`
- SQL changes limited to dialect differences, such as `datetime('now')` becoming `now()`.

## Data Flow

1. User exports selected API configs from SQLite metadata.
2. Export file contains `api_config` rows plus matching `api_sql` rows.
3. User imports a file.
4. Backend validates IDs, paths, group references, and JSON shape.
5. Backend inserts rows into SQLite metadata and invalidates the API config cache.
6. For PostgreSQL demo setup, the app stores only datasource and API metadata in SQLite; runtime requests execute SQL against the PostgreSQL business datasource.

## Error Handling

- Duplicate group names or IDs return a failure message instead of partial import.
- Duplicate API IDs or paths return a failure message instead of partial import.
- Invalid JSON or missing `api`/`sql` arrays returns a clear failure message.
- Missing datasource or group references in imported APIs return a clear failure message.
- PostgreSQL connection failures surface through existing datasource connection and request execution errors.

## Testing

Backend tests:

- Export selected APIs returns `{ api, sql }` with all selected child SQL rows.
- Import rejects duplicate API paths.
- Import inserts API configs and child SQL rows together.
- Group export/import round trip works.
- PostgreSQL URL normalization remains supported by the existing database layer.

Frontend tests/build:

- `npm run build` verifies TypeScript and UI wiring.
- Existing component tests continue to pass.

End-to-end smoke checks:

- `docker compose up -d --build`
- health endpoint returns OK
- PostgreSQL container accepts connections
- Data source list shows the PostgreSQL datasource after setup
- API list can filter by `pg crud`
- `/api/pg/demo/items/qb-list` and `/api/pg/demo/items/view-sql-list` execute against PostgreSQL

## Implementation Order

1. Backend import/export repository functions and routes.
2. Frontend group/import/export controls on the API page.
3. PostgreSQL compose service and demo setup script or seed path.
4. Copy the confirmed demo API template set to `pg crud` with `/pg` prefixes.
5. Build, test, run compose, and smoke test the PG APIs.
