# Design Spec: runtime-rust 3.3.0 Standalone Compatibility

## Context

`runtime-rust` is replacing the Java 3.3.0 standalone backend while reusing the working 3.3.0 Vue frontend from `/Users/andy/IdeaProjects/db-api/dbapi-ui/dist`. The current Rust implementation was based on an older simplified schema and is not compatible with the real 3.3.0 metadata database.

The source of truth for this phase is the Java 3.3.0 standalone app that was verified locally on port `8520`.

## Goals

- Serve the 3.3.0 frontend static files from the Rust process so one standalone service can run the web UI and backend.
- Preserve the real 3.3.0 SQLite metadata schema:
  - `api_config.id` and `datasource.id` are text IDs.
  - API SQL is stored in `api_sql`, not `api_config.sql`.
  - API details include `sqlList`, cache fields, alarm fields, `contentType`, `jsonParam`, and `openTrans`.
- Implement the frontend-facing Java routes needed for the existing UI to load and operate in single-machine mode.
- Keep dynamic `/api/{path}` execution compatible with online API metadata and bound SQL parameters.
- Keep dependencies on the current Rust stack: Rust 2024, Axum 0.8, RBatis/RBDC 4.9, Tokio, Tracing.

## Non-Goals

- No Nacos, gateway, cluster mode, or distributed sync.
- No new frontend work.
- No Java plugin execution. Plugin routes return metadata-compatible placeholder responses where needed by the UI.
- No firewall, token, app authorization, or API permission enforcement in Rust. Existing frontend menu routes return inert compatibility responses only.
- No full Java feature clone for monitoring/Kafka access logs in this phase.
- No unsupported datasource execution for Hive, SQL Server, Elasticsearch, or Oracle.

## Compatibility Contract

### Frontend

Rust serves the copied 3.3.0 `dist` directory. SPA fallback returns `index.html` for browser routes such as `/api`, `/datasource`, and `/login`.

### Auth

The UI must be able to log in with the existing SQLite `user` table. Rust implements:

- `POST /user/login`
- `POST /user/resetPassword`

`Authorization` is accepted for compatibility, but standalone mode does not need a full JWT security model. Routes used by the UI should not fail solely because a Java JWT is absent or differently signed.

### System Routes

Rust implements:

- `POST /system/version` returns `3.3.0-rust`
- `POST /system/mode` returns `standalone`
- `POST /system/getIPPort` returns `127.0.0.1:8520/api`
- `POST /system/getIP` returns `127.0.0.1:8520`

### Metadata Routes

Rust implements these 3.3.0 UI routes:

- `/datasource/add`, `/datasource/update`, `/datasource/getAll`, `/datasource/detail/{id}`, `/datasource/delete/{id}`, `/datasource/connect`
- `/apiConfig/add`, `/apiConfig/update`, `/apiConfig/getAll`, `/apiConfig/search`, `/apiConfig/detail/{id}`, `/apiConfig/delete/{id}`, `/apiConfig/online/{id}`, `/apiConfig/offline/{id}`, `/apiConfig/context`, `/apiConfig/getApiTree`, `/apiConfig/parseParam`, `/apiConfig/parseDynamicSql`, `/apiConfig/sql/execute`
- `/group/create`, `/group/delete/{id}`, `/group/getAll`
- `/plugin/all`
- `/table/getAllTables`, `/table/getAllColumns`
- `/firewall/save`, `/firewall/detail` return inert defaults and do not persist firewall rules.
- `/app/create`, `/app/getAll`, `/app/delete/{id}`, `/app/auth`, `/app/getAuthGroups/{id}` return inert empty compatibility responses and do not implement permissions.

Routes outside this set may return safe empty data or a clear unsupported response.

### Response Shapes

Java-compatible DTOs use:

```json
{
  "success": true,
  "msg": "message",
  "data": null
}
```

Raw list/object/null routes keep Java behavior where the frontend expects it.

### Data Model

Rust JSON field names follow Java frontend names:

- `datasourceId`, `groupId`, `cachePlugin`, `cachePluginParams`
- `contentType`, `jsonParam`, `openTrans`, `createTime`, `updateTime`
- `sqlList[].apiId`, `sqlList[].sqlText`, `sqlList[].transformPlugin`, `sqlList[].transformPluginParams`
- datasource `tableSql`, `createTime`, `updateTime`, `type`, `driver`
- `previlege` is preserved as metadata for UI compatibility but is ignored by Rust runtime authorization.

## Implementation Approach

Use a focused compatibility layer rather than a broad redesign:

- Keep `rbatis` for metadata and datasource execution.
- Add 3.3.0 models for `ApiConfig`, `ApiSql`, `DataSource`, groups, and users. Firewall/app/token authorization models are not part of the Rust runtime domain.
- Add repository helpers that use explicit SQL for the real SQLite schema.
- Split frontend compatibility stubs into small handlers so unsupported single-machine features do not pollute dynamic API execution.
- Copy 3.3.0 dist assets into `runtime-rust/static`.
- Serve static assets and backend routes from one Axum app on port `8520`.

## Testing

- Unit tests for form/json extraction and response DTO naming.
- Repository tests against an in-memory 3.3.0 SQLite schema for text IDs, `api_sql`, and API details.
- Handler tests for login, system routes, datasource CRUD, API CRUD, and group/plugin fallback routes.
- Dynamic API integration test with file-backed SQLite datasource.
- Final verification with `cargo test`, `cargo check`, and smoke curl requests against a running Rust server.
