# Bundle Review Checklist

Every generated bundle must be reviewed before `bundle apply --allow-write`.

## Files To Review

Each bundle directory should contain:

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`

If any file is missing, regenerate the bundle before review.

## Manifest Checks

Review `dbapi_manifest.json`:

- Bundle type matches the request.
- Datasource ID is correct for the target environment.
- Generated API count matches expectations.
- Paths are under the approved `resource_path`.
- API IDs and group IDs are stable and readable.

## Group Checks

Review `api_group_config.json`:

- Group ID is stable.
- Group name is clear to API consumers.
- Group does not collide with unrelated APIs.
- Group ownership is clear from the change record.

## API Config Checks

Review `api_config.json`:

- HTTP methods follow [API Design Standard](API-Design-Standard.md).
- Engine choice follows [SQL and Query Safety](SQL-and-Query-Safety.md).
- Response mode is correct.
- Token/public setting is intentional.
- SQL uses named placeholders such as `$status`.
- QueryBuilder APIs use expected table, filters, sort, limit, and page settings.
- View SQL uses safe filters for structure fragments.

## Curl Checks

Review `curl.md`:

- Examples target the intended base URL.
- Path and method match API config.
- Required params are shown.
- Write examples are safe for the target environment.
- Token examples do not include real production secrets.

## VERIFY Checks

Review `VERIFY.md`:

- It includes server validation.
- It includes at least one success request.
- It includes expected failure or missing-param behavior when relevant.
- It includes response-shape checks.
- It includes access-log confirmation for shared environments.

## Validation Command

Run:

```bash
rtk cargo run -- bundle validate \
  --base-url "$BASE_URL" \
  --dir "$BUNDLE_DIR"
```

Validation must pass before apply.

## Apply Command

Run only after review approval:

```bash
rtk cargo run -- bundle apply \
  --base-url "$BASE_URL" \
  --dir "$BUNDLE_DIR" \
  --allow-write
```

## Review Outcome

Record one of:

| Outcome | Meaning |
| --- | --- |
| Approved | Bundle can be applied to the reviewed environment |
| Approved with environment patch | Bundle can be applied after a reviewed datasource/base-url patch |
| Changes requested | Regenerate or edit the bundle and review again |
| Rejected | API should not be published through DBAPI |
