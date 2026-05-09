# Datasource Management

DBAPI separates metadata storage from business datasources.

- `data.db` stores DBAPI metadata and local demo data.
- SQLite, MySQL, and PostgreSQL datasources provide business data for published APIs.
- Docker Compose mounts the repository `data.db` into the runtime container.

## Supported Datasources

| Datasource | Use |
| --- | --- |
| SQLite | Local demos, lightweight internal data, metadata-backed examples |
| MySQL | Existing MySQL business schemas |
| PostgreSQL | Existing PostgreSQL business schemas and company-standard relational services |

## Required Datasource Information

Each datasource should have:

- Stable datasource ID.
- Human-readable name.
- Database type.
- Connection string or connection fields stored in DBAPI configuration.
- Owner team.
- Environment.
- Read/write permission boundary.

## Connection Validation

Before generating API bundles, validate that the datasource exists and the table schema can be inspected.

For local Docker Compose:

```bash
rtk docker compose up -d --build
rtk curl http://127.0.0.1:8520/health
```

For bundle generation, `draft-table` uses the DBAPI server to inspect the datasource:

```bash
rtk cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id "$DATASOURCE_ID" \
  --table "$TABLE" \
  --resource-path "$RESOURCE_PATH" \
  --group-id "$GROUP_ID" \
  --group-name "$GROUP_NAME" \
  --out "target/dbapi-bundles/$GROUP_ID"
```

## Permission Model

Use least privilege for datasource accounts:

- Query-only APIs should use read-only database users.
- Write APIs should use accounts scoped to required tables.
- Avoid using DBA or application-owner credentials in DBAPI.
- Use separate datasource credentials for dev, staging, and production.

## Metadata Safety

Do not write directly to `data.db` while the runtime has an active SQLite connection. For seed or repair work:

1. Stop the runtime.
2. Apply the seed or repair.
3. Restart the runtime.
4. Run a health check.
5. Check SQLite integrity if corruption is suspected.

Do not commit transient SQLite sidecar files:

```text
data.db-shm
data.db-wal
```

## Environment Mapping

Keep datasource IDs stable where possible. If IDs differ by environment, document the mapping:

| Logical datasource | Dev | Staging | Production |
| --- | --- | --- | --- |
| Orders PostgreSQL | orders_pg_dev | orders_pg_staging | orders_pg_prod |

When promoting bundles, review any datasource ID patch before validation and apply.
