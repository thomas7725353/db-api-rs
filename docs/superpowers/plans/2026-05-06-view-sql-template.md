# View SQL Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a View SQL API mode for complex hand-written SQL templates with safe identifier fragments, bind parameters, limit/offset template parameters, and explicit response modes.

**Architecture:** Keep QueryBuilder and plain SQL behavior unchanged. Add a focused backend `view_sql` renderer using MiniJinja with custom `[[ ... ]]` delimiters and strict filters, then route `transformPlugin=viewSql` API execution through it. Add a frontend `View SQL` editor tab that stores list SQL and optional count SQL in `api_sql` rows, and uses `transform_plugin_params=resultType=<mode>` for the existing return-mode contract.

**Tech Stack:** Rust 2024, Axum, SeaORM/SQLx, SQLParser, MiniJinja 2.19, React 19, Ant Design 6, TypeScript, Vitest.

---

### Task 1: Backend View SQL Renderer

**Files:**
- Modify: `runtime-rust/Cargo.toml`
- Modify: `runtime-rust/src/main.rs`
- Create: `runtime-rust/src/view_sql.rs`

- [ ] **Step 1: Write failing renderer tests**

Create `runtime-rust/src/view_sql.rs` with only the test module and function signatures needed for compilation:

```rust
use anyhow::{Result, anyhow};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedViewSql {
    pub sql: String,
}

pub fn render_view_sql(template: &str, input: &JsonValue) -> Result<RenderedViewSql> {
    let _ = input;
    Err(anyhow!("view sql renderer not implemented: {}", template))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_identifier_list_order_limit_and_offset() {
        let rendered = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]",
            &json!({
                "columns": ["a.id", "a.name", "a.c7"],
                "order_by": "a.c7",
                "limit": 20
            }),
        )
        .unwrap();

        assert_eq!(
            rendered.sql,
            "select a.id, a.name, a.c7 from demo_items order by a.c7 desc limit 20 offset 0"
        );
    }

    #[test]
    fn rejects_unsafe_identifiers() {
        let err = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items",
            &json!({ "columns": ["id", "name; drop table demo_items"] }),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Invalid identifier"));
    }

    #[test]
    fn caps_integer_template_values() {
        let rendered = render_view_sql(
            "limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]",
            &json!({ "limit": 5000, "offset": "3" }),
        )
        .unwrap();

        assert_eq!(rendered.sql, "limit 1000 offset 3");
    }

    #[test]
    fn allows_star_for_qualified_selects() {
        let rendered = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items a",
            &json!({ "columns": ["a.*"] }),
        )
        .unwrap();

        assert_eq!(rendered.sql, "select a.* from demo_items a");
    }
}
```

- [ ] **Step 2: Run renderer tests and verify red**

Run: `rtk cargo test view_sql -- --nocapture`

Expected: tests compile and fail because `render_view_sql` returns `view sql renderer not implemented`.

- [ ] **Step 3: Add MiniJinja dependency and module registration**

In `runtime-rust/Cargo.toml`, add:

```toml
minijinja = { version = "2.19.0", features = ["custom_syntax"] }
```

In `runtime-rust/src/main.rs`, add:

```rust
mod view_sql;
```

- [ ] **Step 4: Implement safe View SQL rendering**

Replace `runtime-rust/src/view_sql.rs` with an implementation that:

```rust
use anyhow::{Result, anyhow};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::{Kwargs, Value};
use minijinja::{Environment, Error, ErrorKind};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedViewSql {
    pub sql: String,
}

pub fn render_view_sql(template: &str, input: &JsonValue) -> Result<RenderedViewSql> {
    let mut env = Environment::new();
    env.set_syntax(
        SyntaxConfig::builder()
            .variable_delimiters("[[", "]]")
            .block_delimiters("[%", "%]")
            .comment_delimiters("[#", "#]")
            .build()?,
    );
    env.add_filter("ident", ident_filter);
    env.add_filter("ident_list", ident_list_filter);
    env.add_filter("int", int_filter);

    let tmpl = env.template_from_str(template)?;
    let sql = tmpl.render(input)?;
    Ok(RenderedViewSql { sql })
}

fn ident_filter(value: Value) -> std::result::Result<String, Error> {
    let raw = value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "identifier must be a string"))?;
    validate_identifier(raw).map_err(template_error)?;
    Ok(raw.trim().to_string())
}

fn ident_list_filter(value: Value) -> std::result::Result<String, Error> {
    let mut parts = Vec::new();
    if let Some(items) = value.try_iter() {
        for item in items {
            let item = item?;
            let raw = item
                .as_str()
                .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "identifier list entries must be strings"))?;
            validate_identifier(raw).map_err(template_error)?;
            parts.push(raw.trim().to_string());
        }
    } else if let Some(raw) = value.as_str() {
        for item in raw.split(',').map(str::trim).filter(|item| !item.is_empty()) {
            validate_identifier(item).map_err(template_error)?;
            parts.push(item.to_string());
        }
    }
    if parts.is_empty() {
        return Err(Error::new(ErrorKind::InvalidOperation, "identifier list cannot be empty"));
    }
    Ok(parts.join(", "))
}

fn int_filter(value: Value, kwargs: Kwargs) -> std::result::Result<String, Error> {
    let default = kwargs.get::<Option<i64>>("default")?.unwrap_or(0);
    let max = kwargs.get::<Option<i64>>("max")?;
    let min = kwargs.get::<Option<i64>>("min")?;
    kwargs.assert_all_used()?;
    let mut parsed = parse_int_value(&value).unwrap_or(default);
    if let Some(min) = min {
        parsed = parsed.max(min);
    }
    if let Some(max) = max {
        parsed = parsed.min(max);
    }
    Ok(parsed.to_string())
}

fn parse_int_value(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
}

fn validate_identifier(raw: &str) -> Result<()> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Invalid identifier: {}", raw));
    }
    if trimmed == "*" {
        return Ok(());
    }
    let mut segments = trimmed.split('.').peekable();
    while let Some(segment) = segments.next() {
        if segment == "*" {
            if segments.peek().is_none() {
                return Ok(());
            }
            return Err(anyhow!("Invalid identifier: {}", raw));
        }
        if !is_identifier_segment(segment) {
            return Err(anyhow!("Invalid identifier: {}", raw));
        }
    }
    Ok(())
}

fn is_identifier_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn template_error(err: anyhow::Error) -> Error {
    Error::new(ErrorKind::InvalidOperation, err.to_string())
}
```

- [ ] **Step 5: Run renderer tests and verify green**

Run: `rtk cargo test view_sql -- --nocapture`

Expected: all `view_sql` tests pass.

- [ ] **Step 6: Commit renderer**

Run:

```bash
rtk git add runtime-rust/Cargo.toml runtime-rust/Cargo.lock runtime-rust/src/main.rs runtime-rust/src/view_sql.rs
rtk git commit -m "feat: add safe view sql template renderer"
```

Expected: one commit containing only renderer-related files.

### Task 2: Runtime Execution Integration

**Files:**
- Modify: `runtime-rust/src/handler.rs`
- Test: `runtime-rust/src/handler.rs`

- [ ] **Step 1: Write failing runtime tests**

Add tests to `runtime-rust/src/handler.rs` under the existing test module:

```rust
#[test]
fn detects_view_sql_config() {
    let config = ApiConfig {
        sql_list: vec![ApiSql {
            transform_plugin: Some("viewSql".to_string()),
            sql_text: Some("select [[ columns | ident_list ]] from demo_items".to_string()),
            transform_plugin_params: Some("resultType=list".to_string()),
            ..empty_api_sql()
        }],
        ..empty_api_config()
    };

    assert!(is_view_sql_config(&config));
}

#[test]
fn finds_view_sql_count_template() {
    let config = ApiConfig {
        sql_list: vec![
            ApiSql {
                transform_plugin: Some("viewSql".to_string()),
                sql_text: Some("select a.* from demo_items a limit [[ limit | int(default=10,max=1000) ]]".to_string()),
                transform_plugin_params: Some("resultType=page".to_string()),
                ..empty_api_sql()
            },
            ApiSql {
                transform_plugin: Some("viewSqlCount".to_string()),
                sql_text: Some("select count(*) as total from demo_items".to_string()),
                transform_plugin_params: None,
                ..empty_api_sql()
            },
        ],
        ..empty_api_config()
    };

    assert_eq!(
        view_sql_count_template(&config).unwrap(),
        "select count(*) as total from demo_items"
    );
}
```

If helpers do not exist in the test module, add:

```rust
fn empty_api_config() -> ApiConfig {
    ApiConfig {
        id: None,
        name: None,
        note: None,
        path: None,
        datasource_id: None,
        sql_list: Vec::new(),
        params: None,
        status: None,
        previlege: None,
        group_id: None,
        cache_plugin: None,
        cache_plugin_params: None,
        create_time: None,
        update_time: None,
        content_type: None,
        open_trans: None,
        json_param: None,
        alarm_plugin: None,
        alarm_plugin_param: None,
    }
}

fn empty_api_sql() -> ApiSql {
    ApiSql {
        id: None,
        api_id: None,
        sql_text: None,
        transform_plugin: None,
        transform_plugin_params: None,
    }
}
```

- [ ] **Step 2: Run runtime tests and verify red**

Run: `rtk cargo test detects_view_sql_config finds_view_sql_count_template -- --nocapture`

Expected: fails because `is_view_sql_config` and `view_sql_count_template` do not exist.

- [ ] **Step 3: Add View SQL execution branch**

In `runtime-rust/src/handler.rs`, import:

```rust
use crate::view_sql;
```

Add helpers next to `is_query_builder_config`:

```rust
fn is_view_sql_config(config: &ApiConfig) -> bool {
    config
        .sql_list
        .first()
        .and_then(|sql| sql.transform_plugin.as_deref())
        .is_some_and(|plugin| plugin.eq_ignore_ascii_case("viewSql"))
}

fn view_sql_count_template(config: &ApiConfig) -> Option<&str> {
    config.sql_list.iter().skip(1).find_map(|sql| {
        let plugin = sql.transform_plugin.as_deref().unwrap_or("");
        if plugin.eq_ignore_ascii_case("viewSqlCount") {
            sql.sql_text.as_deref()
        } else {
            None
        }
    })
}
```

Add a View SQL branch after the QueryBuilder branch and before plain SQL transform:

```rust
if is_view_sql_config(&config) {
    match execute_view_sql(
        &data_db,
        &config,
        sql,
        &all_params,
        dialect,
        first_sql.and_then(|sql| sql.transform_plugin_params.as_deref()),
    )
    .await
    {
        Ok(data) => {
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::OK.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: None,
                },
            )
            .await;
            return api_success(data).into_response();
        }
        Err(e) => {
            let message = e.to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return sql_error(message).into_response();
        }
    }
}
```

Add `execute_view_sql`:

```rust
async fn execute_view_sql(
    data_db: &DbConn,
    config: &ApiConfig,
    template: &str,
    input: &JsonValue,
    dialect: DialectType,
    plugin_params: Option<&str>,
) -> Result<JsonValue> {
    let rendered = view_sql::render_view_sql(template, input)?;
    let (transformed_sql, param_names) = SqlTransformer::transform(&rendered.sql, dialect)?;
    let db_values = bind_param_values(&param_names, input, config.params.as_deref())?;
    let result_type = parse_result_type(plugin_params);

    if result_type == "count" {
        let count_template = view_sql_count_template(config).unwrap_or(template);
        let count_rendered = view_sql::render_view_sql(count_template, input)?;
        let (count_sql, count_param_names) = SqlTransformer::transform(&count_rendered.sql, dialect)?;
        let count_values = bind_param_values(&count_param_names, input, config.params.as_deref())?;
        return Ok(json!(query_builder_total(data_db, &count_sql, count_values).await?));
    }

    let rows = db::query_json(data_db, &transformed_sql, db_values).await?;
    if matches!(result_type.as_str(), "object" | "one" | "single") {
        return Ok(rows.into_iter().next().unwrap_or(JsonValue::Null));
    }

    if result_type == "page" {
        let count_template = view_sql_count_template(config)
            .ok_or_else(|| anyhow!("View SQL page mode requires a count SQL template"))?;
        let count_rendered = view_sql::render_view_sql(count_template, input)?;
        let (count_sql, count_param_names) = SqlTransformer::transform(&count_rendered.sql, dialect)?;
        let count_values = bind_param_values(&count_param_names, input, config.params.as_deref())?;
        let total = query_builder_total(data_db, &count_sql, count_values).await?;
        return Ok(json!({
            "list": rows,
            "total": total,
            "limit": input.get("limit").cloned().unwrap_or(JsonValue::Null),
            "offset": input.get("offset").cloned().unwrap_or(JsonValue::Null)
        }));
    }

    Ok(JsonValue::Array(rows))
}
```

- [ ] **Step 4: Run runtime tests and verify green**

Run: `rtk cargo test detects_view_sql_config finds_view_sql_count_template -- --nocapture`

Expected: both tests pass.

- [ ] **Step 5: Run full Rust tests**

Run: `rtk cargo test`

Expected: all Rust tests pass.

- [ ] **Step 6: Commit runtime integration**

Run:

```bash
rtk git add runtime-rust/src/handler.rs
rtk git commit -m "feat: execute view sql api templates"
```

Expected: one commit with runtime execution integration.

### Task 3: Frontend View SQL Preview Utilities

**Files:**
- Modify: `runtime-rust/frontend/src/api/types.ts`
- Create: `runtime-rust/frontend/src/components/viewSqlPreview.ts`
- Create: `runtime-rust/frontend/src/components/viewSqlPreview.test.ts`

- [ ] **Step 1: Write failing frontend preview tests**

Create `runtime-rust/frontend/src/components/viewSqlPreview.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { renderViewSqlPreview } from './viewSqlPreview';

describe('renderViewSqlPreview', () => {
  it('renders safe identifiers and integer paging fragments', () => {
    const preview = renderViewSqlPreview(
      'select [[ columns | ident_list ]] from demo_items a order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]',
      {
        columns: ['a.id', 'a.name', 'a.c7'],
        order_by: 'a.c7',
        limit: 20,
      },
    );

    expect(preview.sql).toBe('select a.id, a.name, a.c7 from demo_items a order by a.c7 desc limit 20 offset 0');
  });

  it('rejects unsafe identifiers in preview', () => {
    expect(() =>
      renderViewSqlPreview('select [[ columns | ident_list ]] from demo_items', {
        columns: ['id', 'name; drop table demo_items'],
      }),
    ).toThrow(/Invalid identifier/);
  });
});
```

- [ ] **Step 2: Run preview tests and verify red**

Run: `rtk npm test -- --run src/components/viewSqlPreview.test.ts`

Expected: fails because `viewSqlPreview.ts` does not exist.

- [ ] **Step 3: Add View SQL types**

In `runtime-rust/frontend/src/api/types.ts`, change:

```ts
export type ApiEngine = 'sql' | 'queryBuilder';
```

to:

```ts
export type ApiEngine = 'sql' | 'queryBuilder' | 'viewSql';
```

- [ ] **Step 4: Implement preview utility**

Create `runtime-rust/frontend/src/components/viewSqlPreview.ts`:

```ts
export interface ViewSqlPreview {
  sql: string;
}

const VARIABLE_PATTERN = /\[\[\s*([A-Za-z_][A-Za-z0-9_]*)\s*\|\s*([A-Za-z_][A-Za-z0-9_]*)(?:\(([^)]*)\))?\s*\]\]/g;

export function renderViewSqlPreview(template: string, params: Record<string, unknown>): ViewSqlPreview {
  const sql = template.replace(VARIABLE_PATTERN, (_match, name: string, filter: string, args: string | undefined) => {
    const value = params[name];
    if (filter === 'ident') return renderIdent(value);
    if (filter === 'ident_list') return renderIdentList(value);
    if (filter === 'int') return renderInt(value, parseFilterArgs(args));
    throw new Error(`Unsupported filter: ${filter}`);
  });
  return { sql };
}

function renderIdent(value: unknown): string {
  if (typeof value !== 'string') throw new Error('identifier must be a string');
  validateIdentifier(value);
  return value.trim();
}

function renderIdentList(value: unknown): string {
  const values = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value.split(',').map((item) => item.trim()).filter(Boolean)
      : [];
  if (values.length === 0) throw new Error('identifier list cannot be empty');
  return values.map(renderIdent).join(', ');
}

function renderInt(value: unknown, args: Record<string, number>): string {
  const fallback = args.default ?? 0;
  const parsed =
    typeof value === 'number' && Number.isFinite(value)
      ? Math.trunc(value)
      : typeof value === 'string' && /^-?\d+$/.test(value.trim())
        ? Number.parseInt(value.trim(), 10)
        : fallback;
  const min = args.min;
  const max = args.max;
  return String(Math.min(max ?? parsed, Math.max(min ?? parsed, parsed)));
}

function parseFilterArgs(raw: string | undefined): Record<string, number> {
  if (!raw?.trim()) return {};
  const args: Record<string, number> = {};
  for (const part of raw.split(',')) {
    const [key, value] = part.split('=').map((item) => item.trim());
    if (!key || !/^-?\d+$/.test(value ?? '')) continue;
    args[key] = Number.parseInt(value, 10);
  }
  return args;
}

function validateIdentifier(raw: string) {
  const trimmed = raw.trim();
  if (trimmed === '*') return;
  const segments = trimmed.split('.');
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (segment === '*') {
      if (index === segments.length - 1) return;
      throw new Error(`Invalid identifier: ${raw}`);
    }
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(segment)) throw new Error(`Invalid identifier: ${raw}`);
  }
}
```

- [ ] **Step 5: Run preview tests and verify green**

Run: `rtk npm test -- --run src/components/viewSqlPreview.test.ts`

Expected: all preview tests pass.

- [ ] **Step 6: Commit preview utility**

Run:

```bash
rtk git add runtime-rust/frontend/src/api/types.ts runtime-rust/frontend/src/components/viewSqlPreview.ts runtime-rust/frontend/src/components/viewSqlPreview.test.ts
rtk git commit -m "feat: add view sql frontend preview helpers"
```

Expected: one frontend utility commit.

### Task 4: API Editor View SQL Mode

**Files:**
- Modify: `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`

- [ ] **Step 1: Add View SQL state and tab**

In `ApiEditorPage.tsx`, add state:

```ts
const [viewSqlText, setViewSqlText] = useState(
  'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]',
);
const [viewCountSqlText, setViewCountSqlText] = useState('select count(*) as total from demo_items a where a.status = $status');
const [viewPreviewParams, setViewPreviewParams] = useState(
  JSON.stringify(
    {
      columns: ['a.id', 'a.name', 'a.status'],
      order_by: 'a.id',
      limit: 10,
      offset: 0,
      status: 'active',
    },
    null,
    2,
  ),
);
```

Add a `viewSql` tab beside QueryBuilder and SQL with:

```tsx
const viewSqlTab = {
  key: 'viewSql',
  label: 'View SQL',
  children: (
    <div className="space-y-3">
      <Alert
        type="info"
        showIcon
        message="结构片段使用 [[ columns | ident_list ]]、[[ order_by | ident ]]、[[ limit | int(default=10,max=1000) ]]；普通值继续使用 $param 绑定。"
      />
      <Input.TextArea rows={14} value={viewSqlText} onChange={(event) => setViewSqlText(event.target.value)} />
      {(responseMode === 'page' || responseMode === 'count') ? (
        <Input.TextArea
          rows={5}
          value={viewCountSqlText}
          placeholder="page/count 模式需要 count SQL 模板，例如 select count(*) as total from demo_items where status = $status"
          onChange={(event) => setViewCountSqlText(event.target.value)}
        />
      ) : null}
      <Input.TextArea
        rows={6}
        value={viewPreviewParams}
        placeholder='预览参数，例如 {"columns":["a.id"],"order_by":"a.id","limit":10,"offset":0}'
        onChange={(event) => setViewPreviewParams(event.target.value)}
      />
    </div>
  ),
};
```

Ensure `editorTabs` returns `[queryBuilderTab, sqlTab, viewSqlTab]` for new APIs and only the current tab for edit APIs.

- [ ] **Step 2: Load existing View SQL APIs**

In the detail loader, add:

```ts
} else if (firstSql?.transformPlugin === 'viewSql') {
  setEngine('viewSql');
  setViewSqlText(firstSql.sqlText || '');
  setViewCountSqlText(detail.sqlList?.find((item) => item.transformPlugin === 'viewSqlCount')?.sqlText || '');
  setResponseMode(parseResponseMode(firstSql.transformPluginParams));
```

- [ ] **Step 3: Save View SQL APIs**

In `save`, build `sqlList` with three cases:

```ts
const sqlList =
  engine === 'queryBuilder'
    ? [
        {
          sqlText: JSON.stringify(queryBuilderDsl, null, 2),
          transformPlugin: 'queryBuilder',
          transformPluginParams: resultTypeParams(responseMode),
        },
      ]
    : engine === 'viewSql'
      ? [
          {
            sqlText: viewSqlText,
            transformPlugin: 'viewSql',
            transformPluginParams: resultTypeParams(responseMode),
          },
          ...(responseMode === 'page' || responseMode === 'count'
            ? [{ sqlText: viewCountSqlText, transformPlugin: 'viewSqlCount', transformPluginParams: '' }]
            : []),
        ]
      : [{ sqlText, transformPlugin: 'sql', transformPluginParams: '' }];
```

Set params for View SQL:

```ts
params:
  engine === 'queryBuilder'
    ? stringifyParamSpecs(inferQueryBuilderPageParams(queryBuilderDsl))
    : engine === 'viewSql'
      ? stringifyParamSpecs(params)
      : contentType === 'application/json'
        ? '[]'
        : stringifyParamSpecs(params),
```

- [ ] **Step 4: Show return mode for View SQL**

Change:

```tsx
{engine === 'queryBuilder' ? (
```

to:

```tsx
{engine === 'queryBuilder' || engine === 'viewSql' ? (
```

When changing response mode:

```ts
if (engine === 'queryBuilder') {
  setDsl((current) => ({ ...current, count: mode === 'page' || mode === 'count' }));
}
```

- [ ] **Step 5: Show ParamEditor for View SQL**

In the request parameter card, add View SQL before the SQL content-type split:

```tsx
{engine === 'queryBuilder' ? (
  <ParamEditor value={inferQueryBuilderPageParams(sanitizeQueryBuilderDsl(dsl))} readonly emptyText="当前 QueryBuilder 没有分页参数" />
) : engine === 'viewSql' ? (
  <ParamEditor value={params} onChange={setParams} />
) : contentType === 'application/json' ? (
```

- [ ] **Step 6: Build frontend**

Run: `rtk npm run build`

Expected: TypeScript and Vite build pass.

- [ ] **Step 7: Commit editor integration**

Run:

```bash
rtk git add runtime-rust/frontend/src/pages/ApiEditorPage.tsx runtime-rust/static
rtk git commit -m "feat: add view sql api editor mode"
```

Expected: one commit for editor behavior and regenerated static assets.

### Task 5: End-to-End Verification

**Files:**
- No source edits expected unless verification exposes a bug.

- [ ] **Step 1: Run full Rust tests**

Run: `rtk cargo test`

Expected: all Rust tests pass.

- [ ] **Step 2: Run frontend tests**

Run: `rtk npm test -- --run`

Expected: all Vitest tests pass.

- [ ] **Step 3: Run frontend build**

Run: `rtk npm run build`

Expected: TypeScript and Vite build pass.

- [ ] **Step 4: Smoke test server startup**

Run: `rtk cargo run`

Expected: server logs `db-api-rs listening on :8520`. If port `8520` is already in use, stop and report the port conflict instead of killing an unknown process.
