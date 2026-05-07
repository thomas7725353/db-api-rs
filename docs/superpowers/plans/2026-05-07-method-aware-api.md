# Method-Aware API Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add HTTP method semantics to published `/api/{path}` APIs so read APIs can use GET, write APIs use write methods, and GET can never execute mutating SQL.

**Architecture:** Keep the existing unified Axum route and `handle_api` execution pipeline. Add a persisted `method` field to API config, validate the incoming request method near the start of `handle_api`, make parameter extraction method-aware, and adapt the React editor/request/curl UI to use the configured method.

**Tech Stack:** Rust, Axum, SeaORM/SQLx repository helpers, SQLParser-based SQL classification, React, TypeScript, Ant Design, Vitest, Cargo tests.

---

## File Structure

- Modify `src/model.rs`: add `ApiConfig.method` with serde aliases.
- Modify `src/repository.rs`: include `method` in repository columns, inserts, updates, import/export, schema creation, and tests.
- Modify `src/api_config_handler.rs`: parse/normalize/validate method when creating or updating API configs.
- Modify `src/handler.rs`: validate request method, extract params based on method, and reject `GET` for non-query SQL.
- Modify `frontend/src/api/types.ts`: add `ApiMethod` and `ApiConfig.method`.
- Modify `frontend/src/api/services.ts`: make `callUserApi` accept method and encode GET params in the URL.
- Modify `frontend/src/components/curlExample.ts` and `frontend/src/components/curlExample.test.ts`: generate method-aware cURL.
- Modify `frontend/src/pages/ApiEditorPage.tsx`: add method selector and infer default method from engine/SQL.
- Modify `frontend/src/pages/ApiRequestPage.tsx`: show method, send method-aware requests, and generate method-aware cURL.
- Modify demo seed docs only if tests or fixtures depend on fixed POST examples.

## Task 1: Persist API Method

**Files:**
- Modify: `src/model.rs`
- Modify: `src/repository.rs`

- [ ] **Step 1: Add a failing repository test for method round-trip**

Add this test inside `#[cfg(test)] mod tests` in `src/repository.rs` near the existing `select_all_api_configs_includes_sql_list` test:

```rust
#[tokio::test]
async fn api_config_method_round_trips() {
    let db = test_db().await;
    create_api_config_test_tables(&db).await;
    db::execute(
        &db,
        "insert into api_config (id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param, method) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v("api-get"),
            v("demo/items/list"),
            v("Demo List"),
            v(""),
            v("[]"),
            v(1),
            v("ds-1"),
            v(1),
            v("group-1"),
            v(None::<String>),
            v(None::<String>),
            v("2026-05-07 10:00:00"),
            v("2026-05-07 10:00:00"),
            v("application/json"),
            v(0),
            v(None::<String>),
            v("GET"),
        ],
    )
    .await
    .unwrap();

    let config = select_api_by_id(&db, "api-get").await.unwrap().unwrap();

    assert_eq!(config.method.as_deref(), Some("GET"));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
rtk cargo test repository::tests::api_config_method_round_trips
```

Expected: FAIL because `api_config` does not have `method` and `ApiConfig` cannot round-trip it.

- [ ] **Step 3: Add `method` to the Rust model**

In `src/model.rs`, add this field to `ApiConfig` after `path`:

```rust
#[serde(rename = "method", alias = "http_method")]
pub method: Option<String>,
```

- [ ] **Step 4: Update repository column lists and SQL**

Change `API_COLUMNS` in `src/repository.rs` from:

```rust
const API_COLUMNS: &str = "id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param";
```

to:

```rust
const API_COLUMNS: &str = "id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param";
```

Update every `insert into api_config (...) values (...)` statement that writes API config rows to include `method` after `path`.

Example for `insert_api_config`:

```rust
"insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
```

and add `v(&config.method)` immediately after `v(&config.path)`.

Update `update_api_config` from:

```rust
"update api_config set path = ?, name = ?, note = ?, params = ?, status = ?, datasource_id = ?, previlege = ?, group_id = ?, cache_plugin = ?, cache_plugin_params = ?, update_time = ?, content_type = ?, open_trans = ?, json_param = ? where id = ?"
```

to:

```rust
"update api_config set path = ?, method = ?, name = ?, note = ?, params = ?, status = ?, datasource_id = ?, previlege = ?, group_id = ?, cache_plugin = ?, cache_plugin_params = ?, update_time = ?, content_type = ?, open_trans = ?, json_param = ? where id = ?"
```

and add `v(&config.method)` immediately after `v(&config.path)`.

- [ ] **Step 5: Add schema creation and migration support**

In `create_api_config_test_tables`, add:

```sql
method text,
```

after `path text,`.

In the production repository initialization path where `api_config` is created or migrated, ensure the `method` column exists for existing `data.db`. If no helper exists yet, add one near repository initialization:

```rust
async fn ensure_api_config_method_column(db: &DbConn) -> anyhow::Result<()> {
    let _ = db::execute(
        db,
        "alter table api_config add column method text default 'POST'",
        vec![],
    )
    .await;
    Ok(())
}
```

Call it after the existing `api_config` table creation statements. Ignore duplicate-column errors so existing databases keep starting.

- [ ] **Step 6: Fill new field in tests and fixtures**

Every test-created `ApiConfig { ... }` in `src/repository.rs` and `src/handler.rs` must include:

```rust
method: Some("POST".to_string()),
```

For read-focused tests, use:

```rust
method: Some("GET".to_string()),
```

- [ ] **Step 7: Run repository tests**

Run:

```bash
rtk cargo test repository::tests
```

Expected: PASS.

## Task 2: Normalize and Validate API Method in Config Handlers

**Files:**
- Modify: `src/api_config_handler.rs`

- [ ] **Step 1: Add tests for method normalization**

Add these tests inside `#[cfg(test)] mod tests` in `src/api_config_handler.rs`. If the file has no test module, create one at the end of the file.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn api_config_from_input_defaults_method_to_post() {
        let config = api_config_from_input(
            json!({
                "name": "Demo",
                "path": "demo/items/create",
                "datasourceId": "ds-1",
                "sqlList": []
            }),
            false,
        )
        .unwrap();

        assert_eq!(config.method.as_deref(), Some("POST"));
    }

    #[test]
    fn api_config_from_input_uppercases_valid_method() {
        let config = api_config_from_input(
            json!({
                "name": "Demo",
                "path": "demo/items/list",
                "method": "get",
                "datasourceId": "ds-1",
                "sqlList": []
            }),
            false,
        )
        .unwrap();

        assert_eq!(config.method.as_deref(), Some("GET"));
    }

    #[test]
    fn api_config_from_input_rejects_invalid_method() {
        let error = api_config_from_input(
            json!({
                "name": "Demo",
                "path": "demo/items/list",
                "method": "TRACE",
                "datasourceId": "ds-1",
                "sqlList": []
            }),
            false,
        )
        .unwrap_err();

        assert_eq!(error, "Invalid HTTP method");
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test api_config_handler::tests
```

Expected: FAIL because method parsing is not implemented.

- [ ] **Step 3: Add method parsing helper**

Add this helper in `src/api_config_handler.rs` near `normalize_content_params`:

```rust
fn normalize_method(input: &JsonValue) -> Result<String, String> {
    let method = get_string(input, "method")
        .or_else(|| get_string(input, "http_method"))
        .unwrap_or_else(|| "POST".to_string())
        .trim()
        .to_ascii_uppercase();
    match method.as_str() {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" => Ok(method),
        _ => Err("Invalid HTTP method".to_string()),
    }
}
```

- [ ] **Step 4: Store normalized method in `api_config_from_input`**

At the start of `api_config_from_input`, after path validation, add:

```rust
let method = normalize_method(&input)?;
```

Set the `ApiConfig` field:

```rust
method: Some(method),
```

- [ ] **Step 5: Run handler tests**

Run:

```bash
rtk cargo test api_config_handler::tests
```

Expected: PASS.

## Task 3: Enforce Method Semantics in Runtime Handler

**Files:**
- Modify: `src/handler.rs`

- [ ] **Step 1: Add unit tests for method helpers**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src/handler.rs`:

```rust
#[test]
fn configured_method_defaults_to_post() {
    let config = empty_api_config();

    assert_eq!(configured_method(&config), axum::http::Method::POST);
}

#[test]
fn configured_method_reads_uppercase_method() {
    let config = ApiConfig {
        method: Some("GET".to_string()),
        ..empty_api_config()
    };

    assert_eq!(configured_method(&config), axum::http::Method::GET);
}

#[test]
fn validate_request_method_rejects_mismatch() {
    let config = ApiConfig {
        method: Some("GET".to_string()),
        ..empty_api_config()
    };

    let error = validate_request_method(&axum::http::Method::POST, &config).unwrap_err();

    assert_eq!(error, "Method not allowed");
}

#[test]
fn reject_unsafe_get_blocks_mutating_sql() {
    let error = reject_unsafe_get(&axum::http::Method::GET, false).unwrap_err();

    assert_eq!(error, "GET APIs can only execute query SQL");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test handler::tests::configured_method_defaults_to_post handler::tests::validate_request_method_rejects_mismatch handler::tests::reject_unsafe_get_blocks_mutating_sql
```

Expected: FAIL because helper functions do not exist.

- [ ] **Step 3: Capture request method before consuming the request**

At the top of `handle_api`, after `request: Request<Body>`, add:

```rust
let request_method = request.method().clone();
```

- [ ] **Step 4: Add method helper functions**

Add these helpers near `is_query_builder_config`:

```rust
fn configured_method(config: &ApiConfig) -> Method {
    match config
        .method
        .as_deref()
        .unwrap_or("POST")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "GET" => Method::GET,
        "PUT" => Method::PUT,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        _ => Method::POST,
    }
}

fn validate_request_method(method: &Method, config: &ApiConfig) -> Result<(), String> {
    if method == configured_method(config) {
        return Ok(());
    }
    Err("Method not allowed".to_string())
}

fn reject_unsafe_get(method: &Method, is_query: bool) -> Result<(), String> {
    if method == Method::GET && !is_query {
        return Err("GET APIs can only execute query SQL".to_string());
    }
    Ok(())
}
```

Also import `Method`:

```rust
use axum::http::Method;
```

- [ ] **Step 5: Validate method after config load**

After `let api_id = config.id.clone();`, add:

```rust
if let Err(message) = validate_request_method(&request_method, &config) {
    write_access_log(
        &state,
        AccessLogInput {
            url,
            status: StatusCode::METHOD_NOT_ALLOWED.as_u16() as i32,
            duration: started.elapsed().as_millis() as i64,
            timestamp,
            app_id: None,
            api_id,
            error: Some(message.clone()),
        },
    )
    .await;
    return api_error(StatusCode::METHOD_NOT_ALLOWED, message).into_response();
}
```

Make sure later code still has an `api_id` value available. If ownership moves, clone into `api_id_for_log` before returning.

- [ ] **Step 6: Make parameter extraction method-aware**

Replace:

```rust
let body_params = match parse_request_body(request).await {
    ...
};
let all_params = merge_json_objects(map_to_json(query_params), body_params);
```

with:

```rust
let all_params = if request_method == Method::GET {
    map_to_json(query_params)
} else {
    let body_params = match parse_request_body(request).await {
        Ok(params) => params,
        Err(e) => {
            let message = e.to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::BAD_REQUEST.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::BAD_REQUEST, message).into_response();
        }
    };
    merge_json_objects(map_to_json(query_params), body_params)
};
```

- [ ] **Step 7: Reject GET for mutating plain SQL**

After:

```rust
let is_query = SqlTransformer::is_query(sql, dialect).unwrap_or(false);
```

add:

```rust
if let Err(message) = reject_unsafe_get(&request_method, is_query) {
    write_access_log(
        &state,
        AccessLogInput {
            url,
            status: StatusCode::METHOD_NOT_ALLOWED.as_u16() as i32,
            duration: started.elapsed().as_millis() as i64,
            timestamp,
            app_id,
            api_id,
            error: Some(message.clone()),
        },
    )
    .await;
    return api_error(StatusCode::METHOD_NOT_ALLOWED, message).into_response();
}
```

QueryBuilder and View SQL remain GET-capable because they only execute query paths in this codebase.

- [ ] **Step 8: Run handler tests**

Run:

```bash
rtk cargo test handler::tests
```

Expected: PASS.

## Task 4: Make Frontend Types, Requests, and cURL Method-Aware

**Files:**
- Modify: `frontend/src/api/types.ts`
- Modify: `frontend/src/api/services.ts`
- Modify: `frontend/src/components/curlExample.ts`
- Modify: `frontend/src/components/curlExample.test.ts`
- Modify: `frontend/src/pages/ApiRequestPage.tsx`

- [ ] **Step 1: Add failing cURL tests**

Add these tests in `frontend/src/components/curlExample.test.ts`:

```ts
it('generates GET curl with query string parameters', () => {
  const curl = generateCurlCommand({
    method: 'GET',
    url: 'http://127.0.0.1:8520/api/demo/items/list',
    contentType: 'application/json',
    token: 'sk-demo',
    params: [
      { name: 'limit', value: 10 },
      { name: 'offset', value: 0 },
    ],
  });

  expect(curl).toBe(
    [
      "curl 'http://127.0.0.1:8520/api/demo/items/list?limit=10&offset=0' \\",
      "  -H 'Authorization: sk-demo'",
    ].join('\n'),
  );
});

it('generates DELETE curl with query string parameters and no body', () => {
  const curl = generateCurlCommand({
    method: 'DELETE',
    url: 'http://127.0.0.1:8520/api/demo/items/delete',
    contentType: 'application/json',
    params: [{ name: 'id', value: 1 }],
  });

  expect(curl).toBe("curl -X DELETE 'http://127.0.0.1:8520/api/demo/items/delete?id=1'");
});
```

- [ ] **Step 2: Run failing frontend test**

Run:

```bash
rtk npm --prefix frontend test -- --run src/components/curlExample.test.ts
```

Expected: FAIL because `generateCurlCommand` does not accept `method`.

- [ ] **Step 3: Add frontend method types**

In `frontend/src/api/types.ts`, add:

```ts
export type ApiMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
```

Add to `ApiConfig`:

```ts
method?: ApiMethod;
```

- [ ] **Step 4: Update `callUserApi`**

Change the signature in `frontend/src/api/services.ts` to:

```ts
export function callUserApi(
  path: string,
  body: Record<string, unknown>,
  contentType: string,
  token?: string,
  method: ApiMethod = 'POST',
): Promise<unknown> {
```

Import `ApiMethod`.

Implement GET and DELETE query encoding:

```ts
const normalizedMethod = method || 'POST';
const cleanPath = `/api/${path.replace(/^\/+/, '')}`;
const headers: Record<string, string> = {};
if (token) headers.Authorization = token;

if (normalizedMethod === 'GET' || normalizedMethod === 'DELETE') {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(body)) {
    if (Array.isArray(value)) {
      for (const item of value) query.append(key, String(item));
    } else if (value !== undefined && value !== null && value !== '') {
      query.set(key, String(value));
    }
  }
  const suffix = query.toString() ? `?${query.toString()}` : '';
  return apiRequest(`${cleanPath}${suffix}`, { method: normalizedMethod, headers });
}

headers['Content-Type'] = contentType;
const requestBody =
  contentType === 'application/x-www-form-urlencoded'
    ? new URLSearchParams(body as Record<string, string>).toString()
    : typeof body === 'string'
      ? body
      : JSON.stringify(body);
return apiRequest(cleanPath, {
  method: normalizedMethod,
  headers,
  body: requestBody,
});
```

- [ ] **Step 5: Update cURL generation**

Change `CurlCommandInput` in `frontend/src/components/curlExample.ts`:

```ts
method?: ApiMethod;
```

Import `ApiMethod`.

Change `generateCurlCommand` to:

```ts
export function generateCurlCommand(input: CurlCommandInput): string {
  const method = input.method || 'POST';
  const queryParams = method === 'GET' || method === 'DELETE' ? formParts(input.params ?? []) : [];
  const url = queryParams.length ? `${input.url}?${queryParams.map(encodeQueryPart).join('&')}` : input.url;
  const lines = method === 'GET' ? [`curl ${shellQuote(url)}`] : [`curl -X ${method} ${shellQuote(url)}`];

  const token = input.token?.trim();
  if (token) {
    lines.push(`  -H ${shellQuote(`Authorization: ${token}`)}`);
  }

  if (method === 'GET' || method === 'DELETE') {
    return lines.map((line, index) => (index < lines.length - 1 ? `${line} \\` : line)).join('\n');
  }

  lines.push(`  -H ${shellQuote(`Content-Type: ${input.contentType}`)}`);
  if (input.contentType.startsWith('application/x-www-form-urlencoded')) {
    for (const part of formParts(input.params ?? [])) {
      lines.push(`  --data-urlencode ${shellQuote(part)}`);
    }
  } else {
    lines.push(`  --data-raw ${shellQuote(input.body ?? '{}')}`);
  }

  return lines.map((line, index) => (index < lines.length - 1 ? `${line} \\` : line)).join('\n');
}

function encodeQueryPart(part: string): string {
  const [key, ...rest] = part.split('=');
  return `${encodeURIComponent(key)}=${encodeURIComponent(rest.join('='))}`;
}
```

- [ ] **Step 6: Update request page**

In `frontend/src/pages/ApiRequestPage.tsx`, derive:

```ts
const method = detail?.method || 'POST';
```

Pass it into cURL:

```ts
method,
params: method === 'GET' || method === 'DELETE' || !isJson ? params : undefined,
```

Show method near URL:

```tsx
<Form.Item label="Method">
  <Input value={method} readOnly />
</Form.Item>
```

Pass method into send:

```ts
const body = isJson ? JSON.parse(jsonBody || '{}') : paramsToBody(params);
const response = await callUserApi(detail.path, body, contentType, token, method);
```

- [ ] **Step 7: Run frontend tests**

Run:

```bash
rtk npm --prefix frontend test -- --run src/components/curlExample.test.ts
```

Expected: PASS.

## Task 5: Add Method Selector and Defaults in API Editor

**Files:**
- Modify: `frontend/src/pages/ApiEditorPage.tsx`

- [ ] **Step 1: Add method options and SQL inference helper**

Add near the constants:

```ts
const methodOptions = [
  { value: 'GET', label: 'GET：查询' },
  { value: 'POST', label: 'POST：创建 / 兼容' },
  { value: 'PUT', label: 'PUT：替换更新' },
  { value: 'PATCH', label: 'PATCH：局部更新' },
  { value: 'DELETE', label: 'DELETE：删除' },
];

function inferMethod(engine: ApiEngine, sqlText: string): ApiMethod {
  if (engine === 'queryBuilder' || engine === 'viewSql') return 'GET';
  const firstWord = sqlText.trim().split(/\s+/, 1)[0]?.toLowerCase();
  if (firstWord === 'select' || firstWord === 'with' || firstWord === 'show') return 'GET';
  if (firstWord === 'insert') return 'POST';
  if (firstWord === 'update') return 'PATCH';
  if (firstWord === 'delete') return 'DELETE';
  return 'POST';
}
```

Import `ApiMethod` from `../api/types`.

- [ ] **Step 2: Initialize method for new APIs**

Add an effect:

```ts
useEffect(() => {
  if (isEdit) return;
  form.setFieldValue('method', inferMethod(engine, sqlText));
}, [engine, form, isEdit, sqlText]);
```

- [ ] **Step 3: Add method form field**

In the first card grid, add this item after path:

```tsx
<Form.Item name="method" label="Method" initialValue="POST" rules={[{ required: true }]}>
  <Select options={methodOptions} />
</Form.Item>
```

- [ ] **Step 4: Include method in save payload**

In `save`, add:

```ts
method: values.method || inferMethod(engine, sqlText),
```

inside the `payload`.

- [ ] **Step 5: Run frontend build**

Run:

```bash
rtk npm --prefix frontend run build
```

Expected: PASS.

## Task 6: End-to-End Verification and Documentation Touch-Ups

**Files:**
- Modify: `README.md`
- Modify: `skills/dbapi-demo-crud/SKILL.md` only if demo command examples should reflect GET/PATCH/DELETE.

- [ ] **Step 1: Update README API notes**

In `README.md` under `API Execution Notes`, add:

```markdown
- Published APIs have a configured HTTP method. New query APIs should use `GET`; write APIs should use `POST`, `PUT`, `PATCH`, or `DELETE`.
- `GET` requests only read URL query parameters and are rejected if the configured SQL is not a query.
```

- [ ] **Step 2: Run all Rust tests**

Run:

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests and build**

Run:

```bash
rtk npm --prefix frontend test -- --run
rtk npm --prefix frontend run build
```

Expected: PASS.

- [ ] **Step 4: Run manual smoke test**

Start the app if it is not already running:

```bash
rtk cargo run
```

Use another terminal for smoke calls:

```bash
rtk curl -i 'http://127.0.0.1:8520/health'
```

Expected:

```text
HTTP/1.1 200 OK
OK
```

If demo APIs are seeded with GET methods, verify a GET list call succeeds:

```bash
rtk curl -i 'http://127.0.0.1:8520/api/demo/items/list?limit=10&offset=0'
```

Expected: `HTTP/1.1 200 OK` with JSON body.

Verify wrong method returns 405:

```bash
rtk curl -i -X POST 'http://127.0.0.1:8520/api/demo/items/list'
```

Expected: `HTTP/1.1 405 Method Not Allowed`.

- [ ] **Step 5: Commit changes**

Run:

```bash
rtk git status --short
rtk git add src/model.rs src/repository.rs src/api_config_handler.rs src/handler.rs frontend/src/api/types.ts frontend/src/api/services.ts frontend/src/components/curlExample.ts frontend/src/components/curlExample.test.ts frontend/src/pages/ApiEditorPage.tsx frontend/src/pages/ApiRequestPage.tsx README.md docs/superpowers/plans/2026-05-07-method-aware-api.md
rtk git commit -m "feat: add method-aware api execution"
```

Expected: commit succeeds. Do not include unrelated dirty files such as `data.db`, `data.db-shm`, or `data.db-wal` unless the user explicitly asks to update seeded local data.

## Self-Review

- Spec coverage: The plan covers persisted method config, backend validation, GET-only query safety, frontend editor changes, frontend request/curl changes, and verification.
- Placeholder scan: No placeholder steps remain; each code-change task includes concrete snippets and commands.
- Type consistency: Backend uses `method: Option<String>` and frontend uses `ApiMethod`; request runtime normalizes to Axum `Method`.
- Scope check: This does not rewrite db-api-rs into a Directus/PostgREST resource CRUD product. It keeps the existing SQL-publishing model and adds method semantics only.
