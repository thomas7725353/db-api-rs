# SQL and Query Safety

DBAPI supports QueryBuilder, SQL, and View SQL. The company default is to choose the safest engine that satisfies the API requirement.

## Engine Boundaries

| Engine | Best For | Avoid For |
| --- | --- | --- |
| QueryBuilder | Standard list, page, table, filter, count | Complex reports and custom joins |
| SQL | Fixed CRUD and fixed custom queries | Dynamic identifiers or dynamic SQL structure |
| View SQL | Reports with controlled dynamic structure | Free-form user-provided SQL fragments |

## Value Parameters

SQL value parameters must use named placeholders:

```sql
select id, name
from demo_items
where status = $status
```

Do not use positional placeholders:

```sql
select id, name
from demo_items
where status = $1
```

Positional placeholders are rejected by the bundle workflow.

## View SQL Structure Parameters

View SQL uses MiniJinja with `[[ ... ]]` delimiters for safe SQL structure fragments.

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

Values still use `$status` bind parameters. Structure fragments must use safe filters.

Supported filters:

| Filter | Use |
| --- | --- |
| `ident` | One safe identifier such as `id`, `a.id`, or `a.*` |
| `ident_list` | Array or comma-separated list of safe identifiers |
| `int(default=...,max=...,min=...)` | Bounded integer fragment |

Do not template table names. Keep table names in reviewed SQL text or explicit API configuration.

## Read And Write Separation

`GET` APIs must be read-only. Reviewers should check:

- SQL starts with a read query.
- Query does not contain hidden mutation statements.
- Multiple SQL statements are not used.
- Response mode matches the query shape.

Write APIs should use `POST`, `PUT`, `PATCH`, or `DELETE` and should clearly name the action in the path.

## Review Red Flags

Reject or revise an API when SQL contains:

- String concatenation for values.
- User-controlled table names.
- User-controlled raw `order by` text.
- Unbounded `limit`.
- Broad `select *` in production-facing APIs unless intentionally accepted.
- Writes hidden behind `GET`.
- Multiple statements.
- Database-specific privileged operations.

## Safer Defaults

- Prefer QueryBuilder for table list APIs.
- Prefer View SQL over raw SQL string assembly for dynamic reports.
- Set bounded limits for reporting APIs.
- Keep path, method, and response shape stable after consumers adopt the API.
