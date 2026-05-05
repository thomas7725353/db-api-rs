# QueryBuilder Native Operators And Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the API editor QueryBuilder match react-querybuilder's native operator vocabulary, add a `Convert to => SQL/JSON` preview selector, and keep the bottom request parameter definition limited to pagination parameters.

**Architecture:** Keep the existing React/Vite editor and Rust SeaQuery runtime. Frontend operator labels and values should come from react-querybuilder defaults instead of the current hand-written Chinese list. Rust `query_dsl` must accept the same operator values the UI emits, so previewable filters also execute correctly at runtime.

**Tech Stack:** React 19, TypeScript, Ant Design page shell, react-querybuilder 8.15, @react-querybuilder/antd where still useful, Rust 2024, SeaQuery 0.32.

---

### Task 1: Add Backend Coverage For React QueryBuilder Operators

**Files:**
- Modify: `runtime-rust/src/query_dsl.rs`

- [ ] **Step 1: Add failing tests for native string operators**

Append this test inside the existing `#[cfg(test)] mod tests` in `runtime-rust/src/query_dsl.rs`, after `param_object_collects_param_names`:

```rust
    #[test]
    fn supports_react_querybuilder_default_string_operators() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "name", "operator": "doesNotContain", "valueSource": "param", "value": "keyword"},
                    {"field": "name", "operator": "doesNotBeginWith", "value": "Draft"},
                    {"field": "name", "operator": "doesNotEndWith", "value": "Old"}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({"keyword":"Alpha"}), DbBackend::Sqlite).unwrap();

        assert_eq!(built.values.len(), 5);
        assert!(built.sql.contains("NOT LIKE"));
        assert_eq!(db_value_to_json(&built.values[0]), json!("%Alpha%"));
        assert_eq!(db_value_to_json(&built.values[1]), json!("Draft%"));
        assert_eq!(db_value_to_json(&built.values[2]), json!("%Old"));
    }
```

- [ ] **Step 2: Add failing tests for `between` and `notNull`**

Append this test in the same module:

```rust
    #[test]
    fn supports_react_querybuilder_between_and_not_null() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "id", "operator": "between", "value": [10, 20]},
                    {"field": "note", "operator": "notNull", "value": null}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(built.sql.contains("BETWEEN"));
        assert!(built.sql.contains("IS NOT NULL"));
        assert_eq!(db_value_to_json(&built.values[0]), json!(10));
        assert_eq!(db_value_to_json(&built.values[1]), json!(20));
    }
```

- [ ] **Step 3: Run the targeted tests and verify they fail**

Run:

```bash
rtk cargo test --manifest-path runtime-rust/Cargo.toml supports_react_querybuilder
```

Expected: the new tests fail because `doesNotContain`, `doesNotBeginWith`, `doesNotEndWith`, `between`, or `notNull` are not fully supported by `build_rule`.

- [ ] **Step 4: Implement operator support in `build_rule`**

In `runtime-rust/src/query_dsl.rs`, update the null handling before `resolve_value`:

```rust
    if op == "null" || op == "is_null" {
        return Ok(Some(column.is_null()));
    }
    if op == "notnull" || op == "not_null" || op == "is_not_null" {
        return Ok(Some(column.is_null().not()));
    }
```

Then update the `match op.as_str()` block:

```rust
        "contains" | "like" => column.like(like_value(value, LikeMode::Contains)?),
        "doesnotcontain" => column.not_like(like_value(value, LikeMode::Contains)?),
        "begins_with" | "beginswith" => column.like(like_value(value, LikeMode::BeginsWith)?),
        "doesnotbeginwith" => column.not_like(like_value(value, LikeMode::BeginsWith)?),
        "ends_with" | "endswith" => column.like(like_value(value, LikeMode::EndsWith)?),
        "doesnotendwith" => column.not_like(like_value(value, LikeMode::EndsWith)?),
        "in" => column.is_in(json_array_values(value, "in")?),
        "not_in" | "notin" => column.is_in(json_array_values(value, "notIn")?).not(),
        "between" => {
            let (first, second) = json_pair_values(value, "between")?;
            column.between(first, second)
        }
```

Add this helper after `json_array_values`:

```rust
fn json_pair_values(value: JsonValue, op: &str) -> Result<(Value, Value)> {
    let values = match value {
        JsonValue::Array(values) => values,
        JsonValue::String(raw) => raw
            .split(',')
            .map(|value| JsonValue::String(value.trim().to_string()))
            .filter(|value| value.as_str().is_some_and(|raw| !raw.is_empty()))
            .collect(),
        other => {
            return Err(anyhow!(
                "{} operator requires an array or comma separated string value, got {}",
                op,
                other
            ));
        }
    };
    if values.len() < 2 {
        return Err(anyhow!("{} operator requires exactly two values", op));
    }
    Ok((
        db::json_to_db_value(values[0].clone()),
        db::json_to_db_value(values[1].clone()),
    ))
}
```

- [ ] **Step 5: Verify backend tests pass**

Run:

```bash
rtk cargo test --manifest-path runtime-rust/Cargo.toml supports_react_querybuilder
```

Expected: both tests pass.

- [ ] **Step 6: Commit backend operator support**

Run:

```bash
rtk git add runtime-rust/src/query_dsl.rs
rtk git commit -m "feat: support native querybuilder operators"
```

---

### Task 2: Use React QueryBuilder Native Operator List

**Files:**
- Modify: `runtime-rust/frontend/src/components/QueryBuilderEditor.tsx`
- Modify: `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`

- [ ] **Step 1: Replace the hand-written Chinese operator list**

In `runtime-rust/frontend/src/components/QueryBuilderEditor.tsx`, update the import from `react-querybuilder`:

```ts
import {
  QueryBuilder,
  ValueEditor,
  defaultOperators,
  type Field,
  type FullOption,
  type RuleGroupType,
  type ValueEditorProps,
} from 'react-querybuilder';
```

Replace the current `operators` constant with:

```ts
const operators: FullOption[] = defaultOperators.filter((operator) => operator.name !== 'notBetween') as FullOption[];
```

This keeps the native values and labels:

```text
=
!=
<
>
<=
>=
contains
begins with
ends with
does not contain
does not begin with
does not end with
is null
is not null
in
not in
between
```

- [ ] **Step 2: Align custom value editor checks with native operator names**

In `DbApiValueEditor`, change the null and list checks:

```ts
function DbApiValueEditor(props: ValueEditorProps) {
  if (['null', 'notNull', 'not_null'].includes(String(props.operator))) return null;
  if (String(props.valueSource) === 'param') {
    const paramValue = normalizeParamValue(props.value);
    return (
      <Space.Compact>
        <Input
          className="w-44"
          placeholder="param name"
          value={paramValue.param}
          onChange={(event) => props.handleOnChange({ ...paramValue, param: event.target.value })}
        />
        <Input
          className="w-48"
          placeholder="default JSON"
          value={paramValue.defaultText}
          onChange={(event) => {
            const defaultText = event.target.value;
            props.handleOnChange({ ...paramValue, defaultText, default: parseDefault(defaultText) });
          }}
        />
      </Space.Compact>
    );
  }
  if (['in', 'notIn', 'not_in'].includes(String(props.operator))) {
    const value = Array.isArray(props.value) ? props.value.map(String) : splitList(String(props.value ?? ''));
    return <Select mode="tags" className="min-w-64" value={value} tokenSeparators={[',']} onChange={props.handleOnChange} />;
  }
  return <ValueEditor {...props} />;
}
```

- [ ] **Step 3: Align parameter type inference with native operator names**

In `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`, update `inferParamType`:

```ts
function inferParamType(operator: string, value: unknown): ParamSpec['type'] {
  if (['in', 'notIn', 'not_in', 'between'].includes(operator)) return 'Array<string>';
  const defaultValue = value && typeof value === 'object' && 'default' in value ? (value as { default?: unknown }).default : undefined;
  if (typeof defaultValue === 'number') return 'double';
  if (Array.isArray(defaultValue)) return 'Array<string>';
  return 'string';
}
```

- [ ] **Step 4: Run frontend build**

Run:

```bash
cd runtime-rust/frontend && rtk npm run build
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 5: Commit native operator list**

Run:

```bash
rtk git add runtime-rust/frontend/src/components/QueryBuilderEditor.tsx runtime-rust/frontend/src/pages/ApiEditorPage.tsx runtime-rust/static
rtk git commit -m "feat: use native querybuilder operators"
```

---

### Task 3: Add `Convert to => SQL/JSON` Preview Selector

**Files:**
- Modify: `runtime-rust/frontend/src/components/QueryBuilderEditor.tsx`
- Modify: `runtime-rust/frontend/src/styles.css`

- [ ] **Step 1: Add preview imports and state**

In `runtime-rust/frontend/src/components/QueryBuilderEditor.tsx`, add `formatQuery` to the `react-querybuilder` import:

```ts
import {
  QueryBuilder,
  ValueEditor,
  defaultOperators,
  formatQuery,
  type Field,
  type FullOption,
  type RuleGroupType,
  type ValueEditorProps,
} from 'react-querybuilder';
```

Add this type near the component props:

```ts
type PreviewFormat = 'sql' | 'json';
```

Inside `QueryBuilderEditor`, add:

```ts
  const [previewFormat, setPreviewFormat] = useState<PreviewFormat>('sql');
```

- [ ] **Step 2: Add preview text generation**

Inside `QueryBuilderEditor`, before `return`, add:

```ts
  const previewText = useMemo(() => {
    if (previewFormat === 'json') return JSON.stringify(dsl, null, 2);
    return formatQuery(normalizeRulesForPreview(dsl.rules ?? emptyRules), {
      format: 'sql',
      quoteFieldNamesWith: '',
      parseNumbers: 'strict',
    });
  }, [dsl, previewFormat]);
```

Add these helpers near the bottom of the file:

```ts
function normalizeRulesForPreview(group: RuleGroupType): RuleGroupType {
  return {
    ...group,
    rules: (group.rules ?? []).map((node) => {
      if ('rules' in node) return normalizeRulesForPreview(node as RuleGroupType);
      const rule = { ...(node as RuleType & { valueSource?: string; value?: unknown }) };
      if (String(rule.valueSource) === 'param') {
        rule.valueSource = 'value';
        rule.value = previewParamValue(rule.value);
      }
      return rule;
    }),
  };
}

function previewParamValue(value: unknown): string {
  if (typeof value === 'string') return `$${value}`;
  if (value && typeof value === 'object' && 'param' in value) {
    const param = (value as { param?: unknown }).param;
    return typeof param === 'string' && param.trim() ? `$${param.trim()}` : '$param';
  }
  return '$param';
}
```

Also add `RuleType` to the type import from `react-querybuilder`:

```ts
  type RuleType,
```

- [ ] **Step 3: Replace the current Collapse preview UI**

Remove the existing `Collapse` import from Ant Design and remove the current preview block whose collapse item label is `高级预览：生成的 DSL JSON / SQL 规则`.

Insert this preview block at the same position:

```tsx
      <div className="querybuilder-preview">
        <div className="querybuilder-preview-toolbar">
          <Typography.Text className="querybuilder-preview-label">Convert to =&gt;</Typography.Text>
          <Select
            className="querybuilder-preview-select"
            value={previewFormat}
            options={[
              { value: 'sql', label: 'SQL' },
              { value: 'json', label: 'JSON' },
            ]}
            onChange={setPreviewFormat}
          />
        </div>
        <Input.TextArea className="querybuilder-preview-output" rows={previewFormat === 'json' ? 10 : 4} value={previewText} readOnly />
      </div>
```

- [ ] **Step 4: Add focused preview styles**

In `runtime-rust/frontend/src/styles.css`, add:

```css
.querybuilder-preview {
  margin-top: 16px;
}

.querybuilder-preview-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.querybuilder-preview-label {
  font-size: 18px;
  font-style: italic;
}

.querybuilder-preview-select {
  width: 220px;
}

.querybuilder-preview-output textarea {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
}
```

- [ ] **Step 5: Run frontend build**

Run:

```bash
cd runtime-rust/frontend && rtk npm run build
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 6: Commit preview selector**

Run:

```bash
rtk git add runtime-rust/frontend/src/components/QueryBuilderEditor.tsx runtime-rust/frontend/src/styles.css runtime-rust/static
rtk git commit -m "feat: add querybuilder sql json preview"
```

---

### Task 4: Limit Bottom Request Parameter Definition To Pagination

**Files:**
- Modify: `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`

- [ ] **Step 1: Replace QueryBuilder parameter inference for saved metadata**

In `save()`, change the QueryBuilder branch from `inferParams(queryBuilderDsl)` to `inferPageParams(queryBuilderDsl)`:

```ts
      params:
        engine === 'queryBuilder'
          ? stringifyParamSpecs(inferPageParams(queryBuilderDsl))
          : contentType === 'application/json'
            ? '[]'
            : stringifyParamSpecs(params),
```

- [ ] **Step 2: Replace QueryBuilder parameter inference for the bottom table**

In the `请求参数定义` card, change:

```tsx
          <ParamEditor value={inferParams(sanitizeDsl(dsl))} readonly emptyText="当前 QueryBuilder 没有绑定请求参数" />
```

to:

```tsx
          <ParamEditor value={inferPageParams(sanitizeDsl(dsl))} readonly emptyText="当前 QueryBuilder 没有分页请求参数" />
```

- [ ] **Step 3: Add the page-only inference helper**

Replace the existing `inferParams` function with:

```ts
function inferPageParams(dsl: QueryBuilderDsl): ParamSpec[] {
  const params = new Map<string, ParamSpec>();
  if (dsl.limit?.param) params.set(dsl.limit.param, { name: dsl.limit.param, type: 'bigint' });
  if (dsl.offset?.param) params.set(dsl.offset.param, { name: dsl.offset.param, type: 'bigint' });
  return [...params.values()];
}
```

Remove `collectRuleParams`, `extractParamName`, and `inferParamType` if they are no longer referenced after this change.

- [ ] **Step 4: Run frontend build**

Run:

```bash
cd runtime-rust/frontend && rtk npm run build
```

Expected: the build passes, and TypeScript does not report missing symbols.

- [ ] **Step 5: Commit request parameter definition change**

Run:

```bash
rtk git add runtime-rust/frontend/src/pages/ApiEditorPage.tsx runtime-rust/static
rtk git commit -m "fix: show only pagination params for querybuilder"
```

---

### Task 5: Runtime Verification In Browser And API

**Files:**
- No source files should be edited in this task.

- [ ] **Step 1: Run full backend verification**

Run:

```bash
rtk cargo test --manifest-path runtime-rust/Cargo.toml
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run frontend production build**

Run:

```bash
cd runtime-rust/frontend && rtk npm run build
```

Expected: `tsc -b && vite build` succeeds and refreshes `runtime-rust/static`.

- [ ] **Step 3: Start or reuse the local server**

If `http://127.0.0.1:8520` is already serving the app, reuse it. Otherwise run:

```bash
cd runtime-rust && rtk cargo run
```

Expected: the Rust server starts and serves the frontend on `127.0.0.1:8520`.

- [ ] **Step 4: Verify the editor page visually**

Open:

```text
http://127.0.0.1:8520/apis/demo_item_qb_list/edit
```

Expected:
- Operator dropdown labels are English and include `does not contain`, `does not begin with`, `does not end with`, `in`, `not in`, and `between`.
- Operator dropdown does not include `not between`.
- The preview UI is `Convert to =>` plus a dropdown with only `SQL` and `JSON`.
- `SQL` shows generated condition SQL.
- `JSON` shows the current QueryBuilder DSL JSON.
- Bottom `请求参数定义` shows only `limit bigint` and `offset bigint`.
- Bottom `请求参数定义` does not show `keyword` or `status`.

- [ ] **Step 5: Verify QueryBuilder execution still accepts hidden optional filters**

Run a QueryBuilder execution request with `keyword`, `status`, `limit`, and `offset`:

```bash
rtk curl -sS http://127.0.0.1:8520/queryBuilder/execute \
  -X POST \
  -H 'Content-Type: application/json' \
  -d '{"datasourceId":"local_sqlite_demo","dsl":{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true},"params":{"keyword":"Alpha","status":"active","limit":2,"offset":0}}'
```

Expected: the response is successful and returns a page-shaped payload with `list`, `total`, `limit`, and `offset`, proving `keyword` and `status` can remain runtime filter params without being shown in the bottom parameter definition table.

- [ ] **Step 6: Commit verification-only static output if needed**

If Step 2 changed only built static assets that were not included in earlier commits, run:

```bash
rtk git add runtime-rust/static
rtk git commit -m "build: refresh querybuilder frontend assets"
```

If there are no static asset changes left, skip this commit.

---

### Self-Review Notes

- Spec coverage: native English operators are covered by Task 2; runtime support is covered by Task 1; `Convert to => SQL/JSON` is covered by Task 3; bottom parameter filtering is covered by Task 4; browser/API verification is covered by Task 5.
- Scope check: this plan does not modify the `demo_items` database table or direct `data.db` contents.
- Risk note: frontend build writes hashed files under `runtime-rust/static`; commit only the generated asset changes produced by the build for this work.
