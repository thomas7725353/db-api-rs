# Design Spec: runtime-rust Java Compatibility Layer

## Context

The Rust runtime is intended to replace the existing Java/Spring Boot backend while keeping the current Vue frontend in `src/main/webapp` unchanged. The short-term target is a single-machine deployment. Compatibility with the existing frontend, SQLite metadata database, URL paths, request encoding, response shapes, and Java field names is the primary requirement.

This spec supersedes the broad Rust refactor direction for the immediate implementation. The Rust service should behave as a drop-in backend replacement, not as a redesigned API.

## Goals

- Run the existing Vue frontend without frontend changes and without visible behavior differences.
- Preserve the existing `data.db` schema and data semantics.
- Implement the Java management API surface used by the frontend.
- Keep `/api/{path}` dynamic SQL execution compatible with current configured APIs.
- Upgrade the Rust runtime to current stable dependencies: Rust 2024 edition, Axum 0.8, RBatis/RBDC 4.9 series, and the tracing ecosystem.
- Support single-machine metadata, cache, and connection-pool behavior.

## Non-Goals

- No new frontend.
- No distributed mode or cluster coordination.
- No auth redesign.
- No schema migration beyond preserving and reading the existing `datasource` and `api_config` tables.
- No short-term Hive or SQL Server driver implementation. The existing frontend options can remain visible, but Rust should return a clear unsupported-datasource response for these types.

## Compatibility Contract

### HTTP Routes

Rust must expose the same backend routes currently used by the Vue app:

- `GET /health`
- `ANY /api/{path}` for online dynamic API execution.
- `POST /apiConfig/add`
- `POST /apiConfig/update`
- `GET /apiConfig/getAll`
- `GET /apiConfig/detail/{id}`
- `GET /apiConfig/delete/{id}`
- `GET /apiConfig/online/{id}`
- `GET /apiConfig/offline/{id}`
- `POST /apiConfig/parseParam`
- `GET /apiConfig/getIPPort`
- `POST /apiConfig/request`
- `POST /datasource/add`
- `POST /datasource/update`
- `GET /datasource/getAll`
- `GET /datasource/detail/{id}`
- `GET /datasource/delete/{id}`
- `POST /datasource/connect`

### Request Encoding

The Vue frontend globally sends POST requests as `application/x-www-form-urlencoded`. Rust must parse form-urlencoded bodies for every management endpoint and for dynamic `/api/{path}` calls. JSON request bodies may remain supported as a secondary convenience, but form-urlencoded is the compatibility path.

### Response Shapes

Rust must serialize API and data source models using Java-compatible field names:

- `ApiConfig.datasourceId`, not `datasource_id`.
- `ApiConfig.params` as the existing string payload, not a JSON value in management responses.
- `DataSource.type`, not `db_type`.

Management mutation responses should follow the Java `ResponseDto` shape where the frontend expects it:

```json
{
  "success": true,
  "message": "message text",
  "data": null
}
```

Endpoints that Java returned as raw lists, raw objects, or `null` should keep that behavior unless the frontend explicitly expects `ResponseDto`.

### Metadata Tables

Rust must use the existing tables without requiring a migration:

```sql
datasource(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT,
  note TEXT,
  type TEXT,
  url TEXT,
  username TEXT,
  password TEXT
)
```

```sql
api_config(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT,
  name TEXT,
  note TEXT,
  sql TEXT,
  params TEXT,
  status INTEGER,
  datasource_id INTEGER
)
```

## Architecture

### App State

`AppState` owns:

- Metadata `RBatis` connected to local SQLite `data.db`.
- A datasource `PoolManager`.
- A short-lived API config cache keyed by path.

The cache must be invalidated after `apiConfig` add, update, delete, online, and offline operations. Datasource pools must be invalidated after datasource update or delete. A short TTL can exist as a backup, but correctness must not depend on waiting for TTL expiry after admin changes.

### Management Services

Split management behavior out of the dynamic API handler:

- `api_config_handler`: route handlers for `/apiConfig/*`.
- `datasource_handler`: route handlers for `/datasource/*`.
- `form`: shared extractor helpers for form-urlencoded and optional JSON.
- `response`: Java-compatible DTO helpers.

This avoids turning the existing dynamic execution handler into an oversized file.

### Dynamic API Execution

`/api/{path}` should:

1. Normalize the requested path to match Java's bare `path` lookup.
2. Load only online `api_config` rows with `status = 1`.
3. Parse parameters from query string, form-urlencoded body, or JSON body.
4. Validate required parameters from the stored `params` metadata string.
5. Convert `$param` placeholders into driver bind placeholders.
6. Execute prepared SQL through the datasource pool.
7. Return result rows as JSON.

Unlike Java's direct string replacement, Rust should preserve the safer bind-parameter implementation. This is an intentional compatibility-safe improvement because the frontend observes only request and response behavior, not the SQL construction mechanism.

### Datasource Support

Supported in the immediate Rust single-machine version:

- `mysql`
- `postgreSql`
- `postgres`
- `postgresql`
- `sqlite`

Unsupported but explicitly handled:

- `hive`
- `sqlServer`

Unsupported types should not panic or leak internal errors. `/datasource/connect` should return a Java-compatible failed `ResponseDto` with a clear message.

### Dependency Upgrade

Update the Rust runtime dependency baseline:

- `edition = "2024"`
- `axum = "0.8.9"`
- `tower-http = "0.6.8"`
- `rbatis = "4.9.3"`
- `rbs = "4.8.4"`
- `rbdc-sqlite = "4.9.5"`
- `rbdc-mysql = "4.9.5"`
- `rbdc-pg = "4.9.5"`
- Remove direct `log` and `env_logger`.
- Add `tracing` and `tracing-subscriber`.

Code should use `tracing::{info, warn, error, instrument}` where useful and initialize logging through `tracing_subscriber`.

## Error Handling

- Do not leak raw backend driver errors to dynamic API clients.
- Management endpoints may return concise failure messages matching Java's operational style.
- Duplicate API paths must be rejected on add and update.
- Datasource deletion must fail if any `api_config` row still references it.
- Invalid `params` metadata must fail clearly during parse or execution.
- Unknown parameter types should be rejected instead of silently falling back.

## Testing Strategy

Use TDD for implementation changes.

Required coverage:

- Form-urlencoded extraction for management endpoints and dynamic `/api/{path}`.
- Java-compatible `ApiConfig` serialization: `datasourceId` and string `params`.
- `/apiConfig/add`, duplicate path rejection, update resets status to offline, online/offline, delete.
- `/datasource/add`, update, delete refusal when referenced, connect unsupported type handling.
- Cache invalidation after API config mutations.
- Pool invalidation after datasource update/delete.
- SQL placeholder binding for MySQL, PostgreSQL, and SQLite placeholder styles.
- End-to-end smoke test against local SQLite metadata and a SQLite target datasource.

Final verification should include:

- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo check`
- A lightweight webapp compatibility smoke path proving the Vue backend calls can hit Rust routes without 404 or content-type failures.

## Acceptance Criteria

- The Vue app can keep its current request code and backend proxy target.
- All frontend-used Java endpoints exist in Rust.
- Existing `data.db` can be read without migration.
- Frontend-visible field names match Java behavior.
- POST form-urlencoded requests work.
- Dynamic online APIs execute through Rust.
- Admin changes are immediately reflected in runtime execution without restarting Rust.
- Rust runtime builds and tests cleanly on the upgraded dependency stack.
