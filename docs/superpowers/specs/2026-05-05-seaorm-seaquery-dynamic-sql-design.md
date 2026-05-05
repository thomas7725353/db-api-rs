# SeaORM/SeaQuery Dynamic SQL Design

## Goal

Replace the Rust runtime database access layer with SeaORM/SeaQuery while preserving DBAPI 3.3.0 standalone compatibility, then add a runtime dynamic SQL engine compatible with the existing Vue 3.3.0 SQL editor. A SeaQuery-based gendry-like table query DSL is included as a backend MVP only; a full rewritten frontend rule engine is explicitly out of this phase.

## Scope

In scope:

- Replace `rbatis`, `rbs`, `rbdc-sqlite`, `rbdc-mysql`, and `rbdc-pg` with SeaORM backed by SQLx.
- Keep `sqlparser` for validating user-authored SQL and determining query versus DML statements.
- Add a small execution abstraction over SeaORM:
  - `query_json(sql, values) -> Vec<serde_json::Value>`
  - `execute(sql, values) -> rows_affected`
  - `query_one_json(sql, values) -> Option<serde_json::Value>`
- Preserve all existing admin endpoints, token auth, access logging, demo CRUD APIs, and 3.3.0 static frontend behavior.
- Implement runtime dynamic SQL support for DBAPI-compatible templates:
  - `<if test="">`
  - `<where>`
  - `<trim prefix="" suffix="" prefixOverrides="" suffixOverrides="">`
  - `<foreach collection="" item="" index="" open="" close="" separator="">`
  - `#{param}` bind values
  - constrained `${param}` text substitution for numeric and identifier fragments only
- Implement `/apiConfig/parseDynamicSql` and `/apiConfig/sql/execute` for the existing SQL editor.
- Add a backend-only SeaQuery table query MVP endpoint that can build safe SQL from a structured JSON DSL.

Out of scope:

- Rewriting the current Vue 3.3.0 frontend.
- Building a visual rule engine UI.
- Reintroducing Nacos/cluster mode.
- Full plugin, cache, alarm, and transform plugin execution.

## Architecture

The runtime will use a single database layer module to hide SeaORM details from handlers and repositories. `repository.rs` will call this wrapper for metadata operations. `handler.rs` will use the same wrapper for user API execution against configured datasources.

Dynamic SQL is a separate module. It receives a SQL template and request parameters and returns a prepared SQL string plus SeaQuery values. The module is independent of SeaORM so it can be tested without a database.

The optional table-query MVP is another separate module that converts a JSON query DSL to SeaQuery AST. It shares identifier validation and value conversion with dynamic SQL but does not replace dynamic SQL.

## Runtime SQL Model

For dynamic SQL, templates stored in `api_sql.sql_text` are evaluated at request time:

```xml
SELECT id, name, status, note
FROM demo_items
<where>
  <if test="keyword != null and keyword != ''">
    AND (name LIKE #{keywordLike} OR note LIKE #{keywordLike})
  </if>
  <if test="status != null and status != ''">
    AND status = #{status}
  </if>
</where>
ORDER BY id DESC
LIMIT #{limit} OFFSET #{offset}
```

The evaluated SQL uses bind placeholders appropriate for the target database. Values from `#{...}` are never interpolated into the SQL text. Values from `${...}` are rejected unless they match the safe literal policy.

## Safety Rules

- Multiple SQL statements are rejected with `sqlparser`.
- Only `SELECT`, `INSERT`, `UPDATE`, and `DELETE` are allowed for user APIs.
- `${...}` is denied by default unless the value is one of:
  - integer or decimal literal
  - safe identifier: `[A-Za-z_][A-Za-z0-9_]*`
  - comma-separated safe identifiers
- Field/table names in the SeaQuery DSL must come from datasource metadata or an explicit allowlist.
- `limit` and `offset` are capped by backend defaults.

## Compatibility

Existing 3.3.0 frontend endpoints remain:

- `/apiConfig/parseDynamicSql`
- `/apiConfig/sql/execute`
- `/api/{path}`
- `/table/getAllTables`
- `/table/getAllColumns`
- token/app/access-log endpoints

The response shape remains DBAPI-compatible: user API calls return `success/msg/data`, SQL editor endpoints return `ResponseDto` style JSON.

## Testing

Unit tests:

- Dynamic SQL `<if>` condition inclusion/exclusion.
- `<where>` removes leading `AND`/`OR`.
- `<trim>` applies prefix/suffix and override behavior.
- `<foreach>` builds `IN (?, ?)` style lists.
- `#{}` binds values and `${}` rejects unsafe text.
- SeaQuery DSL builds expected SQL and values.

Integration tests:

- SeaORM wrapper can create/query/update an in-memory SQLite database.
- Demo CRUD list with token returns expected rows.
- `/apiConfig/parseDynamicSql` returns rendered SQL and parameters.
- `/apiConfig/sql/execute` can run dynamic SQL against the local SQLite datasource.
- Access logs still record 401 and 200 API calls.

