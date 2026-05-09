# API Design Standard

This page defines the company API design rules for DBAPI endpoints.

## Path Rules

Published APIs live under:

```text
/api/{path}
```

The `resource_path` must be explicit. Do not infer it from table names, API names, or SQL text.

Recommended path shape:

```text
{domain}/{system-or-source}/{resource}/{action}
```

Examples:

```text
ops/postgres/items/qb-list
finance/mysql/invoices/table
hr/report/headcount/view-sql-list
```

Avoid:

- Single-word root paths such as `list`.
- Table names without domain context.
- Paths that expose internal schema names when they are not meaningful to users.
- Reusing the same path for different semantics across environments.

## Method Rules

Use HTTP methods consistently:

| Method | Use |
| --- | --- |
| GET | Read-only query, object lookup, list, page, count |
| POST | Create or command-style operation |
| PUT | Full update by key |
| PATCH | Partial update |
| DELETE | Delete by key |

`GET` APIs must be read-only. Runtime method checks reject mutating SQL through `GET`, but review should catch the design problem before apply.

## Engine Selection

Use the smallest engine that fits the API:

| Need | Engine |
| --- | --- |
| Standard list, filter, table, page, count | QueryBuilder |
| Fixed create/get/update/delete by primary key | SQL |
| One fixed custom query | SQL |
| Report, join, dynamic columns, dynamic order, bounded limit/offset | View SQL |

The default generated table bundle follows this split:

- `create`, `get`, `update`, `delete`: SQL
- `qb-list`, `table`: QueryBuilder
- `view-sql-list`: View SQL

## Response Modes

Use response modes based on consumer expectations:

| Mode | Use |
| --- | --- |
| `list` | Return an array |
| `page` | Return `{ list, total, limit, offset }` |
| `object` | Return one row object |
| `count` | Return only the total |

For UI tables, prefer `page` when the row count matters and `list` when the result set is intentionally small.

## Naming Rules

API IDs and group IDs should be stable and environment-neutral. Prefer lowercase words joined by underscores:

```text
pg_demo_items_group
pg_demo_items_qb_list
finance_invoice_table
```

Display names can be human-readable:

```text
PG Demo Items
Finance Invoice Table
```

Do not encode environment names such as `dev`, `staging`, or `prod` into API IDs unless the API itself is environment-specific.

## Token Policy

Use public APIs only for local demos or explicitly approved internal endpoints behind trusted network controls.

For shared environments:

- Prefer token-protected APIs.
- Keep tokens outside Git.
- Rotate tokens when consumers or ownership change.
- Record token owner and intended consumers in the change record.

## Compatibility Rule

Changing path, method, response mode, or response field names is a breaking change. Create a new API path when existing consumers cannot be migrated in lockstep.
