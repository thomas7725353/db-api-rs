# db-api-rs Company GitOps Wiki

`db-api-rs` is the company standard candidate for publishing internal database-backed HTTP APIs with a reviewable GitOps workflow.

It provides:

- A Rust runtime for published APIs under `/api/{path}`.
- A React admin UI for datasource, API, token, and log management.
- Bundle commands for generating, validating, and applying API configuration files.
- Repo-local agent skills and an MCP sidecar for repeatable AI-assisted API creation.
- Support for SQLite, MySQL, and PostgreSQL datasources.

This wiki is the company operating guide. It is intentionally stricter than the project README. The README explains how the project works; this wiki defines how teams should use it safely and consistently.

## Standard Flow

All production API changes should follow this path:

```text
request
  -> inspect datasource and table
  -> generate bundle files
  -> review bundle files in Git
  -> validate bundle against target DBAPI server
  -> apply with explicit write permission
  -> run generated curl and VERIFY checks
  -> promote the same reviewed files across environments
```

Direct manual edits in the UI are acceptable for local exploration, but they are not the company standard for shared or production environments.

## Wiki Pages

- [GitOps Workflow](GitOps-Workflow.md)
- [API Design Standard](API-Design-Standard.md)
- [Datasource Management](Datasource-Management.md)
- [SQL and Query Safety](SQL-and-Query-Safety.md)
- [Bundle Review Checklist](Bundle-Review-Checklist.md)
- [Environment Promotion](Environment-Promotion.md)
- [Agent and MCP Workflow](Agent-and-MCP-Workflow.md)
- [Operations and Audit](Operations-and-Audit.md)
- [FAQ](FAQ.md)

## Adoption Policy

Use `db-api-rs` when the team needs to expose internal database queries quickly and still keep API changes reviewable. Good fits include:

- Internal tools and back-office systems.
- Admin tables and CRUD-like data services.
- Reporting, operational dashboards, and data lookup APIs.
- Prototypes that may later move to a dedicated service.

Do not use it as the default for:

- Public internet APIs with complex product contracts.
- Cross-service business workflows that need domain transactions.
- High-volume write paths where a dedicated service needs explicit performance engineering.
- APIs that require custom authorization beyond DBAPI token protection and upstream network controls.

## Source Of Truth

For company rollout, treat the following artifacts as source of truth:

- Git repository: API bundle files, review records, and environment promotion history.
- `dbapi_manifest.json`: generated API inventory for a bundle.
- `api_group_config.json`: group import payload.
- `api_config.json`: API import payload.
- `curl.md`: runnable API examples.
- `VERIFY.md`: verification checklist generated with the bundle.
- DBAPI access logs: runtime evidence for published API calls.

If the live UI configuration differs from Git-reviewed bundle files, reconcile it before promoting the API to another environment.
