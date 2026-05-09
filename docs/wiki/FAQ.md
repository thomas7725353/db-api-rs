# FAQ

## Is the README enough for company rollout?

No. The README explains project usage. Company rollout also needs governance: path rules, method rules, SQL safety, review checkpoints, promotion, rollback, and audit evidence.

## Can teams create APIs directly in the UI?

For local exploration, yes. For shared environments and production, no. The company standard is to generate bundle files, review them in Git, validate, apply, and verify.

## Why not let agents apply changes directly?

Agents can draft and validate quickly, but API changes affect shared runtime behavior. Human review is required before `bundle apply --allow-write`.

## Why is `resource_path` explicit?

Path names are product contracts. Inferring paths from table names creates unstable and schema-leaking APIs. The requester or API owner must choose the path.

## When should I use QueryBuilder?

Use QueryBuilder for common table list, filter, page, count, and table APIs. It is the default for generated `qb-list` and `table` endpoints.

## When should I use SQL?

Use SQL for fixed create/get/update/delete by primary key or a fixed custom query. Use named parameters such as `$status`.

## When should I use View SQL?

Use View SQL for reporting or analysis APIs that need controlled dynamic columns, sort fields, limit, offset, or joins.

## Can GET write data?

No. `GET` is read-only. Write APIs must use `POST`, `PUT`, `PATCH`, or `DELETE`.

## Can SQL use `$1` positional parameters?

No. Use named parameters such as `$status`. Positional placeholders are rejected by the bundle workflow.

## Can table names be dynamic in View SQL?

No. Table names should remain in reviewed SQL text or explicit API configuration. View SQL only allows controlled identifier and integer fragments through safe filters.

## How do we promote an API to production?

Promote the same reviewed bundle files. Validate against production, apply with `--allow-write`, run `curl.md`, complete `VERIFY.md`, and record evidence.

## How do we roll back?

Reapply the previous reviewed bundle or disable the affected API, depending on the incident. Rollback must include evidence of the previous bundle and post-rollback verification.

## Where should real tokens live?

Outside Git. Use environment-specific secret storage or operational token management. Examples in `curl.md` must not contain real production tokens.
