# db-api-rs

`db-api-rs` is a standalone Rust runtime and React admin UI for publishing database queries as HTTP APIs. It is designed for fast internal API creation over SQLite, MySQL, and PostgreSQL data sources.

The runtime lives at the repository root and is implemented with Axum, SeaORM/SQLx, SQLParser, SeaQuery, MiniJinja, React, and Ant Design.

## Features

- Manage data sources from the web UI.
- Create and publish API endpoints under `/api/{path}`.
- Support public APIs and token-protected APIs.
- Record access logs for monitoring.
- Build APIs in three modes:
  - `QueryBuilder`: structured query DSL for common list/filter/count APIs.
  - `SQL`: hand-written SQL with `$param` bind parameters.
  - `View SQL`: complex SQL templates for joins, dynamic select columns, sort columns, limit, and offset.
- Return response modes:
  - `list`: return an array.
  - `page`: return `{ list, total, limit, offset }`.
  - `object`: return a single row object.
  - `count`: return only the total.

## View SQL Templates

View SQL uses MiniJinja with custom `[[ ... ]]` delimiters for safe SQL structure fragments. Normal values should still use `$param` bind parameters.

Example:

```sql
select [[ columns | ident_list ]]
from demo_items a
inner join demo_items b
  on a.id >= b.id
where b.status = $status
order by [[ order_by | ident ]] desc
limit [[ limit | int(default=10,max=1000) ]]
offset [[ offset | int(default=0) ]]
```

Preview parameters:

```json
{
  "columns": ["a.id", "a.name", "a.status"],
  "order_by": "a.id",
  "limit": 10,
  "offset": 0,
  "status": "active"
}
```

Rendered SQL:

```sql
select a.id, a.name, a.status
from demo_items a
inner join demo_items b
  on a.id >= b.id
where b.status = ?
order by a.id desc
limit 10
offset 0
```

Supported filters:

- `ident`: a safe identifier such as `id`, `a.id`, or `a.*`.
- `ident_list`: an array or comma-separated list of safe identifiers.
- `int(default=...,max=...,min=...)`: an integer fragment with optional bounds.

Table names are intentionally not templated. Keep table names in the SQL text or in explicit API configuration.

## Quick Start

Build and run with Docker Compose:

```bash
docker compose up -d --build
```

Open:

```text
http://127.0.0.1:8520
```

Health check:

```bash
curl http://127.0.0.1:8520/health
```

The default compose file mounts the repository `data.db` into the container:

```yaml
volumes:
  - ./data.db:/data/data.db
```

## Local Development

Run the Rust backend:

```bash
cargo run
```

Run the frontend dev server:

```bash
cd frontend
npm install
npm run dev
```

Build frontend static assets:

```bash
cd frontend
npm run build
```

Run checks:

```bash
cargo test

cd frontend
npm test -- --run
npm run build
```

## Project Layout

```text
src/                    Rust runtime source
frontend/               React admin UI
static/                 Built frontend assets served by the runtime
seed_demo_api.sql       Demo API seed SQL
data.db                 Local SQLite metadata and demo database
Dockerfile              Container image build
docker-compose.yml      Local container deployment
docs/superpowers/       Planning documents used during development
```

## API Execution Notes

- SQL value parameters use `$name` in stored SQL and are bound by the backend.
- View SQL structure parameters are limited to safe identifiers and integers.
- Multiple SQL statements are rejected by the runtime SQL transformer.
- QueryBuilder uses SeaQuery to generate SQL for the selected database backend.
- Access logs are written for successful and failed `/api/{path}` calls.

## Repository

Canonical GitHub repository:

```text
https://github.com/thomas7725353/db-api-rs
```
