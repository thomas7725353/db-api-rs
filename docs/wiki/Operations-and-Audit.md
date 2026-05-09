# Operations and Audit

This page defines the runtime checks and audit evidence expected for company DBAPI deployments.

## Health Checks

Local Docker Compose:

```bash
rtk docker compose up -d --build
rtk curl http://127.0.0.1:8520/health
```

The expected health endpoint is:

```text
/health
```

## Access Logs

DBAPI records access logs for `/api/{path}` calls, including successful and failed calls.

For shared environments, verification should include:

- One successful request.
- One expected failure when relevant, such as missing required parameter.
- Confirmation that the access log records the request.

## Token Operations

Token-protected APIs should be the default for shared environments unless the network boundary is already sufficient and the owner approves public access.

Operational rules:

- Do not commit real tokens.
- Use environment-specific token values.
- Rotate tokens when ownership or consumers change.
- Record intended consumers.
- Prefer short-lived or scoped tokens when the deployment model supports it.

## Runtime Changes

Before changing production API behavior:

1. Export or locate the previous reviewed bundle.
2. Generate or update the new bundle.
3. Review differences in Git.
4. Validate against production.
5. Apply with explicit approval.
6. Run verification.
7. Record evidence.

## Incident Response

For API incidents, collect:

- API path and method.
- API ID and group ID.
- Datasource ID.
- Current bundle files.
- Last known good bundle files.
- Access log samples.
- Verification commands and outputs.
- Any datasource connection errors.

Then decide whether to:

- Reapply the last known good bundle.
- Disable the affected API.
- Rotate token.
- Fix datasource credentials.
- Patch SQL and promote a new reviewed bundle.

## SQLite Metadata Safety

If metadata corruption is suspected:

1. Stop the DBAPI runtime.
2. Do not write additional seed data while the runtime is active.
3. Check SQLite integrity.
4. Restore from a known good `data.db` or regenerate metadata from reviewed bundles.
5. Restart the runtime.
6. Verify health and critical APIs.

## Release And Deployment Evidence

For company rollout, keep evidence for:

- Runtime version or Git commit.
- Container image or binary artifact.
- `data.db` migration or seed source when used.
- Bundle commit hash.
- Validation output.
- Apply operator.
- Verification output.
