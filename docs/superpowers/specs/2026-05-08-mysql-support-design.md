# MySQL Productized Datasource Support Design

## Goal

Make MySQL a productized business datasource in `db-api-rs`, at the same local-demo quality level as the current PostgreSQL support. SQLite remains the metadata database. MySQL is used only for business tables and runtime API execution.

## Scope

- Keep `DB_API_METADATA_URL` pointed at SQLite metadata storage.
- Treat MySQL as a first-class datasource in runtime execution, QueryBuilder, schema inspection, and admin connection testing.
- Add a MySQL service to `docker-compose.yml` for local demos.
- Add a MySQL init script that creates and seeds a `demo_items` business table.
- Add a SQLite metadata seed script for a `mysql_demo` datasource, `mysql_crud_group`, and `/mysql/demo/items/*` CRUD APIs.
- Verify MySQL with real local container execution and curl smoke checks.

## Non-Goals

- Do not migrate metadata storage from SQLite to MySQL.
- Do not remove or rewrite existing SQLite or PostgreSQL demos.
- Do not introduce a new datasource abstraction layer in this phase.
- Do not support MySQL-specific advanced features such as stored procedures, multiple statements per request, or vendor-specific schema namespaces beyond the current database.
- Do not change the public API import/export format.

## Current State

The codebase already has several MySQL-ready pieces:

- `Cargo.toml` enables SeaORM `sqlx-mysql`.
- `src/db.rs` normalizes `mysql://` and `jdbc:mysql://` URLs.
- `src/sql_engine.rs` has a MySQL dialect and uses `?` placeholders for MySQL.
- `src/query_dsl.rs` builds QueryBuilder SQL with `MysqlQueryBuilder`.
- `src/schema.rs` has `DbBackend::MySql` table and column inspection paths.
- `frontend/src/pages/DatasourcesPage.tsx` already lists MySQL in the datasource type selector.

The gap is productization: datasource connection testing is not a real DB connection check, compose has no MySQL service, there is no MySQL demo table or metadata seed, and the runtime path lacks an end-to-end MySQL proof.

## Backend Design

Update datasource connection testing so `/datasource/connect` opens a real connection for supported datasource types:

- Parse the submitted datasource payload through the existing `DataSource` model path.
- Normalize the datasource URL with the same logic used by runtime pools.
- Attempt a SeaORM connection and a cheap validation query.
- Return a DBAPI-style success only when the connection succeeds.
- Return a clear failure message with the underlying connection error when it fails.

Keep the runtime execution path unchanged where possible:

- API execution continues to select dialect from datasource `type`.
- MySQL SQL templates use `:param` authoring and are transformed to `?` placeholders.
- QueryBuilder continues to choose SQL builder from `DbConn.backend`.
- Schema inspection continues to query `information_schema` for the active database with `database()`.

If any helper currently needed by `/datasource/connect` is private, expose a narrow function from `src/db.rs` rather than duplicating URL normalization.

## Docker And Demo Data

Extend `docker-compose.yml` with a MySQL service:

- image: `mysql:8`
- database: `dbapi_demo`
- user: `dbapi`
- password: `dbapi_pass`
- root password: local demo value
- host port: `127.0.0.1:13306:3306`
- container healthcheck using `mysqladmin ping`
- init volume: `./docker/mysql/init:/docker-entrypoint-initdb.d:ro`

Add `docker/mysql/init/001-demo-items.sql`:

- Create table `demo_items`.
- Use an auto-increment integer primary key.
- Include columns compatible with the existing demo APIs: `name`, `status`, `note`, `created_at`, `updated_at`.
- Seed a small number of deterministic rows.

Update the `db-api-rs` service dependency so the app starts after both PostgreSQL and MySQL are healthy.

## Metadata Seed Design

Add `seed_mysql_demo_api.sql` for the SQLite metadata database. It should be deterministic and safe to re-run by deleting or replacing the known demo IDs before insertion.

Seed records:

- datasource: `mysql_demo`
- group: `mysql_crud_group`
- API configs under `/mysql/demo/items/*`
- child SQL rows for list, get, create, update, and delete

Use MySQL-compatible SQL in the seeded API definitions:

- `select ... from demo_items ... limit :limit offset :offset`
- `insert into demo_items (...) values (...)`
- `update demo_items set ..., updated_at = current_timestamp where id = :id`
- `delete from demo_items where id = :id`
- optional create response can use `last_insert_id()` only if represented as a query API; otherwise keep the existing execute response with `rowsAffected`.

The seed should mirror the PostgreSQL demo shape where practical so API behavior is easy to compare across database backends.

## Data Flow

1. User starts the compose stack.
2. MySQL initializes the `dbapi_demo` business database and `demo_items` table.
3. User applies `seed_mysql_demo_api.sql` to SQLite metadata.
4. Admin UI shows the MySQL datasource and `mysql_crud_group` APIs.
5. Runtime request to `/api/mysql/demo/items/*` loads metadata from SQLite.
6. Runtime opens or reuses a MySQL pool for `mysql_demo`.
7. SQL transformer converts named params to MySQL placeholders.
8. SeaORM executes against MySQL and returns DBAPI JSON responses.

## Error Handling

- Unsupported datasource types continue returning the existing unsupported-type error.
- MySQL connection failures return a failure envelope instead of a false success.
- Invalid JDBC/native MySQL URL input returns a normalization or connection error.
- Missing MySQL datasource metadata returns the existing datasource-not-found runtime error.
- MySQL SQL execution failures return the existing SQL error response and are written to access logs.
- Re-running the seed script should not create duplicate demo datasources, groups, or API paths.

## Testing

Backend tests:

- MySQL URL normalization supports native and JDBC forms with username/password fields.
- Datasource type normalization recognizes `mysql`.
- SQL transformer keeps MySQL placeholders as `?`.
- QueryBuilder preview for MySQL produces MySQL-compatible SQL.
- Schema helper parsing recognizes MySQL primary key, nullable, default, and auto-increment metadata.

Frontend tests/build:

- Existing frontend tests continue to pass.
- `npm run build` verifies the datasource page and API editor still compile.

End-to-end smoke checks:

- `docker compose up -d --build`
- MySQL container reports healthy.
- DBAPI health endpoint responds.
- Apply `seed_mysql_demo_api.sql` to `data.db`.
- `/datasource/connect` succeeds for the MySQL demo datasource and fails for bad credentials.
- Curl checks cover list, get, create, update, and delete under `/api/mysql/demo/items/*`.
- Existing PostgreSQL demo endpoints still work after adding MySQL to compose.

## Implementation Order

1. Expose a narrow real datasource connection-test helper in `src/db.rs`.
2. Update `/datasource/connect` to use the helper.
3. Add MySQL compose service and init SQL.
4. Add deterministic MySQL metadata seed SQL.
5. Add or adjust focused backend tests for connection normalization, MySQL SQL generation, and schema parsing.
6. Run Rust tests, frontend tests/build, compose health checks, and MySQL curl smoke checks.
