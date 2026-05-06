# API Import/Export and PG CRUD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore DBAPI API/group import-export, keep SQLite metadata storage, and add a PostgreSQL business datasource demo copied from the confirmed Demo Item API template set.

**Architecture:** SQLite remains the metadata database. Backend import/export operates on datasource-agnostic metadata rows and exposes old-compatible routes. Frontend API management gains generic group, import, and export controls. PostgreSQL is added through Docker Compose as a business datasource and receives a copied `pg crud` API set with `/pg` path prefixes.

**Tech Stack:** Rust, Axum, SeaORM/SQLx, SeaQuery, SQLite metadata, PostgreSQL business datasource, React, Ant Design, Vite.

---

## File Structure

- Modify `Cargo.toml`: enable Axum multipart extraction.
- Modify `src/model.rs`: add export/import bundle structs used by handlers and repository tests.
- Modify `src/repository.rs`: add datasource-agnostic batch select, duplicate validation, and insert helpers for API configs, API SQL, alarms, and groups.
- Modify `src/api_config_handler.rs`: add old-compatible API/group import, export, and docs handlers.
- Modify `src/main.rs`: register new `/apiConfig/*` routes.
- Modify `frontend/src/api/client.ts`: add raw blob download and multipart upload helpers.
- Modify `frontend/src/api/types.ts`: add API export bundle and API tree node types.
- Modify `frontend/src/api/services.ts`: add API/group import-export service methods.
- Modify `frontend/src/pages/ApisPage.tsx`: add group filtering, group management, import/export controls, and export tree modal.
- Modify `docker-compose.yml`: add PostgreSQL business datasource service and DBAPI dependency.
- Create `docker/postgres/init/001-demo-items.sql`: initialize PostgreSQL demo table/data.
- Create `seed_pg_demo_api.sql`: reproducibly seed SQLite metadata for the PostgreSQL datasource, `pg crud` group, and copied `/pg` APIs.
- Modify `data.db`: apply the PG metadata seed so the current UI shows the new PG datasource and API group after startup.

Do not create feature branches. Project workflow requires direct commits to `main` and push to `origin/main`.

---

### Task 1: Backend API/Group Export and Import

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/model.rs`
- Modify: `src/repository.rs`
- Modify: `src/api_config_handler.rs`
- Modify: `src/main.rs`
- Test: `src/repository.rs`

- [ ] **Step 1: Enable multipart support**

Change the Axum dependency in `Cargo.toml` from:

```toml
axum = "0.8.9"
```

to:

```toml
axum = { version = "0.8.9", features = ["multipart"] }
```

- [ ] **Step 2: Add typed export bundle structs**

In `src/model.rs`, after `ApiGroup`, add:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfigExport {
    #[serde(default)]
    pub api: Vec<ApiConfig>,
    #[serde(default)]
    pub sql: Vec<ApiSql>,
}
```

Keep `ApiConfig` unchanged so normal list/detail responses still include `sqlList`.

- [ ] **Step 3: Add repository batch helpers**

In `src/repository.rs`, extend the model import:

```rust
use crate::model::{
    AccessLog, ApiAlarm, ApiConfig, ApiConfigExport, ApiGroup, ApiSql, AppInfo, DataSource, User,
};
```

Add these functions after `select_all_api_configs`:

```rust
pub async fn export_api_configs(db: &DbConn, ids: &[String]) -> anyhow::Result<ApiConfigExport> {
    let mut api = Vec::new();
    let mut sql = Vec::new();
    for id in ids {
        if let Some(mut config) = select_api_by_id(db, id).await? {
            let sql_rows = select_api_sqls(db, id).await?;
            sql.extend(sql_rows.clone());
            config.sql_list = Vec::new();
            api.push(config);
        }
    }
    Ok(ApiConfigExport { api, sql })
}

pub async fn select_groups_by_ids(db: &DbConn, ids: &[String]) -> anyhow::Result<Vec<ApiGroup>> {
    let mut groups = Vec::new();
    for id in ids {
        if let Some(group) = db::query_one_as(
            db,
            "select id, name from api_group where id = ?",
            vec![v(id)],
        )
        .await?
        {
            groups.push(group);
        }
    }
    Ok(groups)
}

pub async fn import_groups(db: &DbConn, groups: &[ApiGroup]) -> anyhow::Result<()> {
    validate_import_groups(db, groups).await?;
    for group in groups {
        insert_group(db, group).await?;
    }
    Ok(())
}

pub async fn import_api_configs(db: &DbConn, bundle: &ApiConfigExport) -> anyhow::Result<()> {
    validate_import_api_configs(db, bundle).await?;
    for config in &bundle.api {
        let mut config_without_children = config.clone();
        config_without_children.sql_list = Vec::new();
        insert_api_config(db, &config_without_children).await?;
    }
    for sql in &bundle.sql {
        db::execute(
            db,
            "insert into api_sql (api_id, sql_text, transform_plugin, transform_plugin_params) values (?, ?, ?, ?)",
            vec![
                v(&sql.api_id),
                v(&sql.sql_text),
                v(&sql.transform_plugin),
                v(&sql.transform_plugin_params),
            ],
        )
        .await?;
    }
    Ok(())
}
```

Then add validation helpers near the new functions:

```rust
async fn validate_import_groups(db: &DbConn, groups: &[ApiGroup]) -> anyhow::Result<()> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    for group in groups {
        let id = group.id.as_deref().unwrap_or("").trim();
        let name = group.name.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            anyhow::bail!("group id is required");
        }
        if name.is_empty() {
            anyhow::bail!("group name is required");
        }
        if !seen_ids.insert(id.to_string()) {
            anyhow::bail!("duplicate group id in import file: {}", id);
        }
        if !seen_names.insert(name.to_string()) {
            anyhow::bail!("duplicate group name in import file: {}", name);
        }
        if count_first(db, "select count(1) from api_group where id = ?", vec![v(id)]).await > 0 {
            anyhow::bail!("group id already exists: {}", id);
        }
        if count_first(db, "select count(1) from api_group where name = ?", vec![v(name)]).await > 0 {
            anyhow::bail!("group name already exists: {}", name);
        }
    }
    Ok(())
}

async fn validate_import_api_configs(db: &DbConn, bundle: &ApiConfigExport) -> anyhow::Result<()> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();
    for config in &bundle.api {
        let id = config.id.as_deref().unwrap_or("").trim();
        let path = config.path.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            anyhow::bail!("api id is required");
        }
        if path.is_empty() {
            anyhow::bail!("api path is required");
        }
        if !seen_ids.insert(id.to_string()) {
            anyhow::bail!("duplicate api id in import file: {}", id);
        }
        if !seen_paths.insert(path.to_string()) {
            anyhow::bail!("duplicate api path in import file: {}", path);
        }
        if count_first(db, "select count(1) from api_config where id = ?", vec![v(id)]).await > 0 {
            anyhow::bail!("api id already exists: {}", id);
        }
        if count_first(db, "select count(1) from api_config where path = ?", vec![v(path)]).await > 0 {
            anyhow::bail!("api path already exists: {}", path);
        }
        if let Some(group_id) = config.group_id.as_deref().filter(|value| !value.trim().is_empty()) {
            if count_first(db, "select count(1) from api_group where id = ?", vec![v(group_id)]).await == 0 {
                anyhow::bail!("api group does not exist: {}", group_id);
            }
        }
        if let Some(datasource_id) = config.datasource_id.as_deref().filter(|value| !value.trim().is_empty()) {
            if count_first(db, "select count(1) from datasource where id = ?", vec![v(datasource_id)]).await == 0 {
                anyhow::bail!("datasource does not exist: {}", datasource_id);
            }
        }
    }
    Ok(())
}
```

This validates MySQL, PostgreSQL, and SQLite imports generically because it checks metadata references, not database type.

- [ ] **Step 4: Add repository tests**

In `src/repository.rs` tests, extend `create_api_config_test_tables` to also create datasource and group tables. Add tests:

```rust
#[tokio::test]
async fn export_api_configs_returns_old_compatible_bundle() {
    let db = init_repository("sqlite::memory:").await.unwrap();
    create_api_config_test_tables(&db).await;
    db::execute(&db, "insert into api_group (id, name) values (?, ?)", vec![v("group-1"), v("demo")]).await.unwrap();
    db::execute(&db, "insert into datasource (id, name, type, url, username, password, driver) values (?, ?, ?, ?, ?, ?, ?)", vec![v("ds-1"), v("SQLite"), v("sqlite"), v("sqlite::memory:"), v(""), v(""), v("org.sqlite.JDBC")]).await.unwrap();
    db::execute(
        &db,
        "insert into api_config (id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![v("api-1"), v("/demo/items/get"), v("get"), v(""), v("[]"), v(1), v("ds-1"), v(0), v("group-1"), v(Option::<String>::None), v(Option::<String>::None), v("2026-05-06 00:00:00"), v("2026-05-06 00:00:00"), v("application/x-www-form-urlencoded"), v(0), v(Option::<String>::None)],
    ).await.unwrap();
    db::execute(&db, "insert into api_sql (api_id, sql_text, transform_plugin, transform_plugin_params) values (?, ?, ?, ?)", vec![v("api-1"), v("select 1"), v("sql"), v("")]).await.unwrap();

    let bundle = export_api_configs(&db, &["api-1".to_string()]).await.unwrap();

    assert_eq!(bundle.api.len(), 1);
    assert_eq!(bundle.sql.len(), 1);
    assert!(bundle.api[0].sql_list.is_empty());
    assert_eq!(bundle.sql[0].api_id.as_deref(), Some("api-1"));
}

#[tokio::test]
async fn import_api_configs_rejects_duplicate_path() {
    let db = init_repository("sqlite::memory:").await.unwrap();
    create_api_config_test_tables(&db).await;
    db::execute(&db, "insert into api_group (id, name) values (?, ?)", vec![v("group-1"), v("demo")]).await.unwrap();
    db::execute(&db, "insert into datasource (id, name, type, url, username, password, driver) values (?, ?, ?, ?, ?, ?, ?)", vec![v("ds-1"), v("SQLite"), v("sqlite"), v("sqlite::memory:"), v(""), v(""), v("org.sqlite.JDBC")]).await.unwrap();
    db::execute(
        &db,
        "insert into api_config (id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![v("existing"), v("/demo/items/get"), v("existing"), v(""), v("[]"), v(1), v("ds-1"), v(0), v("group-1"), v(Option::<String>::None), v(Option::<String>::None), v("2026-05-06 00:00:00"), v("2026-05-06 00:00:00"), v("application/x-www-form-urlencoded"), v(0), v(Option::<String>::None)],
    ).await.unwrap();
    let bundle = ApiConfigExport {
        api: vec![ApiConfig {
            id: Some("api-2".to_string()),
            name: Some("new".to_string()),
            note: None,
            path: Some("/demo/items/get".to_string()),
            datasource_id: Some("ds-1".to_string()),
            sql_list: vec![],
            params: Some("[]".to_string()),
            status: Some(0),
            previlege: Some(0),
            group_id: Some("group-1".to_string()),
            cache_plugin: None,
            cache_plugin_params: None,
            create_time: Some("2026-05-06 00:00:00".to_string()),
            update_time: Some("2026-05-06 00:00:00".to_string()),
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            open_trans: Some(0),
            json_param: None,
            alarm_plugin: None,
            alarm_plugin_param: None,
        }],
        sql: vec![],
    };

    let err = import_api_configs(&db, &bundle).await.unwrap_err();

    assert!(err.to_string().contains("api path already exists"));
}
```

Also update `create_api_config_test_tables` to create these tables:

```rust
db::execute(db, "create table api_group (id text primary key, name text unique not null)", vec![]).await.unwrap();
db::execute(db, "create table datasource (id text primary key, name text, note text, type text, url text, username text, password text, driver text not null, table_sql text, create_time text, update_time text)", vec![]).await.unwrap();
```

- [ ] **Step 5: Add backend handlers**

In `src/api_config_handler.rs`, update imports:

```rust
use crate::model::{ApiConfig, ApiConfigExport, ApiGroup, ApiSql};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderValue, Request, header},
    response::{IntoResponse, Response},
};
```

Add query type:

```rust
#[derive(Debug, Deserialize)]
pub struct IdsQuery {
    ids: Option<String>,
}
```

Add handlers before `parse_param`:

```rust
pub async fn download_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IdsQuery>,
) -> impl IntoResponse {
    let ids = split_ids(query.ids.as_deref());
    match repository::export_api_configs(&state.metadata_db, &ids).await {
        Ok(bundle) => match serde_json::to_string_pretty(&bundle) {
            Ok(content) => download_text("api_config.json", "application/json", content).into_response(),
            Err(e) => dto_fail(e.to_string()).into_response(),
        },
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn import_config(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> impl IntoResponse {
    let value = match read_json_upload(multipart).await {
        Ok(value) => value,
        Err(e) => return dto_fail(e).into_response(),
    };
    let bundle = match serde_json::from_value::<ApiConfigExport>(value) {
        Ok(bundle) => bundle,
        Err(e) => return dto_fail(format!("Invalid API config JSON: {}", e)).into_response(),
    };
    match repository::import_api_configs(&state.metadata_db, &bundle).await {
        Ok(_) => {
            state.config_cache.invalidate_all();
            dto_ok::<JsonValue>("Import Success", None).into_response()
        }
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn download_group_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IdsQuery>,
) -> impl IntoResponse {
    let ids = split_ids(query.ids.as_deref());
    match repository::select_groups_by_ids(&state.metadata_db, &ids).await {
        Ok(groups) => match serde_json::to_string_pretty(&groups) {
            Ok(content) => download_text("api_group_config.json", "application/json", content).into_response(),
            Err(e) => dto_fail(e.to_string()).into_response(),
        },
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn import_group(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> impl IntoResponse {
    let value = match read_json_upload(multipart).await {
        Ok(value) => value,
        Err(e) => return dto_fail(e).into_response(),
    };
    let groups = match serde_json::from_value::<Vec<ApiGroup>>(value) {
        Ok(groups) => groups,
        Err(e) => return dto_fail(format!("Invalid API group JSON: {}", e)).into_response(),
    };
    match repository::import_groups(&state.metadata_db, &groups).await {
        Ok(_) => dto_ok::<JsonValue>("Import Success", None).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}
```

Add API docs handler:

```rust
pub async fn api_docs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IdsQuery>,
) -> impl IntoResponse {
    let ids = split_ids(query.ids.as_deref());
    match repository::export_api_configs(&state.metadata_db, &ids).await {
        Ok(bundle) => download_text("API Doc.md", "text/markdown; charset=utf-8", render_api_docs(&bundle)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}
```

Add helpers at the bottom:

```rust
async fn read_json_upload(mut multipart: Multipart) -> Result<JsonValue, String> {
    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let bytes = field.bytes().await.map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            continue;
        }
        return serde_json::from_slice::<JsonValue>(&bytes).map_err(|e| e.to_string());
    }
    Err("file is required".to_string())
}

fn split_ids(ids: Option<&str>) -> Vec<String> {
    ids.unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

fn download_text(filename: &str, content_type: &'static str, content: String) -> Response {
    let mut response = content.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition);
    response
}

fn render_api_docs(bundle: &ApiConfigExport) -> String {
    let mut markdown = String::from("# 接口文档\n---\n");
    for api in &bundle.api {
        markdown.push_str(&format!(
            "## {}\n- 接口地址： /api/{}\n- 接口备注：{}\n- Content-Type：{}\n",
            api.name.as_deref().unwrap_or(""),
            api.path.as_deref().unwrap_or("").trim_start_matches('/'),
            api.note.as_deref().unwrap_or(""),
            api.content_type.as_deref().unwrap_or("")
        ));
        markdown.push_str("- 请求参数：\n");
        if api.content_type.as_deref() == Some("application/json") {
            markdown.push_str("```json\n");
            markdown.push_str(api.json_param.as_deref().unwrap_or("{}"));
            markdown.push_str("\n```\n");
        } else {
            markdown.push_str(&render_param_table(api.params.as_deref().unwrap_or("[]")));
        }
        markdown.push_str("\n---\n");
    }
    markdown.push_str(&format!("\n导出日期：{}", repository::now_string()));
    markdown
}

fn render_param_table(raw: &str) -> String {
    let params = serde_json::from_str::<Vec<JsonValue>>(raw).unwrap_or_default();
    if params.is_empty() {
        return "无参数\n".to_string();
    }
    let mut table = String::from("\n| 参数名称 | 参数类型 | 参数说明 |\n| :----: | :----: | :----: |\n");
    for param in params {
        table.push_str(&format!(
            "|{}|{}|{}|\n",
            param.get("name").and_then(JsonValue::as_str).unwrap_or(""),
            param.get("type").and_then(JsonValue::as_str).unwrap_or(""),
            param.get("note").and_then(JsonValue::as_str).unwrap_or("")
        ));
    }
    table
}
```

- [ ] **Step 6: Register routes**

In `src/main.rs`, after `/apiConfig/getApiTree`, add:

```rust
.route(
    "/apiConfig/downloadConfig",
    post(api_config_handler::download_config),
)
.route("/apiConfig/import", post(api_config_handler::import_config))
.route(
    "/apiConfig/downloadGroupConfig",
    post(api_config_handler::download_group_config),
)
.route(
    "/apiConfig/importGroup",
    post(api_config_handler::import_group),
)
.route("/apiConfig/apiDocs", post(api_config_handler::api_docs))
```

- [ ] **Step 7: Run backend tests**

Run:

```bash
rtk cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 8: Commit backend work**

Run:

```bash
rtk git add Cargo.toml Cargo.lock src/model.rs src/repository.rs src/api_config_handler.rs src/main.rs
rtk git commit -m "feat: add api group import export backend"
```

Expected: one commit is created.

---

### Task 2: Frontend Group, Import, and Export Controls

**Files:**
- Modify: `frontend/src/api/client.ts`
- Modify: `frontend/src/api/types.ts`
- Modify: `frontend/src/api/services.ts`
- Modify: `frontend/src/pages/ApisPage.tsx`

- [ ] **Step 1: Add raw download and upload helpers**

In `frontend/src/api/client.ts`, add after `apiGet`:

```ts
export async function apiDownload(path: string, init: RequestInit = {}): Promise<Blob> {
  const response = await fetch(path, init);
  if (!response.ok) {
    const text = await response.text();
    const payload = parsePayload(text);
    throw new ApiError(extractMessage(payload) ?? response.statusText, response.status, payload);
  }
  return response.blob();
}

export async function apiUpload<T>(path: string, file: File): Promise<T> {
  const formData = new FormData();
  formData.append('file', file);
  return apiRequest<T>(path, {
    method: 'POST',
    body: formData,
  });
}
```

- [ ] **Step 2: Add frontend types**

In `frontend/src/api/types.ts`, add after `ApiGroup`:

```ts
export interface ApiConfigExport {
  api: ApiConfig[];
  sql: ApiSql[];
}

export interface ApiTreeNode {
  name: string;
  id?: string;
  children?: ApiConfig[];
}
```

- [ ] **Step 3: Add service methods**

In `frontend/src/api/services.ts`, update import:

```ts
import { apiDownload, apiGet, apiPost, apiRequest, apiUpload } from './client';
import type { AccessLog, ApiConfig, ApiGroup, ApiTreeNode, AppInfo, DataSource, TableColumn } from './types';
```

Add helpers above `apiConfigService`:

```ts
function idsQuery(ids: string[]): string {
  return `ids=${encodeURIComponent(ids.join(','))}`;
}

export function downloadBlob(blob: Blob, filename: string) {
  const link = document.createElement('a');
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(link.href);
}
```

Extend `apiConfigService`:

```ts
  tree: () => apiPost<ApiTreeNode[]>('/apiConfig/getApiTree'),
  exportConfig: (ids: string[]) =>
    apiDownload(`/apiConfig/downloadConfig?${idsQuery(ids)}`, { method: 'POST' }),
  exportDocs: (ids: string[]) =>
    apiDownload(`/apiConfig/apiDocs?${idsQuery(ids)}`, { method: 'POST' }),
  importConfig: (file: File) => apiUpload<unknown>('/apiConfig/import', file),
  exportGroups: (ids: string[]) =>
    apiDownload(`/apiConfig/downloadGroupConfig?${idsQuery(ids)}`, { method: 'POST' }),
  importGroups: (file: File) => apiUpload<unknown>('/apiConfig/importGroup', file),
```

- [ ] **Step 4: Replace the API page toolbar state**

In `frontend/src/pages/ApisPage.tsx`, update imports:

```tsx
import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  FileTextOutlined,
  FolderOutlined,
  PlayCircleOutlined,
  PlusOutlined,
  ReloadOutlined,
  UploadOutlined,
} from '@ant-design/icons';
import { App, Button, Input, Modal, Popconfirm, Select, Space, Table, Tag, Tree, Typography } from 'antd';
import { useEffect, useMemo, useRef, useState } from 'react';
import { apiConfigService, downloadBlob, groupService } from '../api/services';
import type { ApiConfig, ApiGroup, ApiTreeNode } from '../api/types';
```

Inside the component, replace state with:

```tsx
const [rows, setRows] = useState<ApiConfig[]>([]);
const [groups, setGroups] = useState<ApiGroup[]>([]);
const [tree, setTree] = useState<ApiTreeNode[]>([]);
const [keyword, setKeyword] = useState('');
const [field, setField] = useState<string | undefined>();
const [groupId, setGroupId] = useState<string | undefined>();
const [loading, setLoading] = useState(false);
const [groupModalOpen, setGroupModalOpen] = useState(false);
const [newGroupName, setNewGroupName] = useState('');
const [exportOpen, setExportOpen] = useState(false);
const [exportMode, setExportMode] = useState<'api' | 'docs'>('api');
const [checkedApiIds, setCheckedApiIds] = useState<React.Key[]>([]);
const [groupExportOpen, setGroupExportOpen] = useState(false);
const [checkedGroupIds, setCheckedGroupIds] = useState<string[]>([]);
const apiImportRef = useRef<HTMLInputElement>(null);
const groupImportRef = useRef<HTMLInputElement>(null);
```

- [ ] **Step 5: Update load and filter behavior**

Replace `load` with:

```tsx
async function load() {
  setLoading(true);
  try {
    const hasFilter = Boolean(keyword || field || groupId);
    const [nextRows, nextGroups] = await Promise.all([
      hasFilter ? apiConfigService.search({ keyword, field, groupId }) : apiConfigService.list(),
      groupService.list(),
    ]);
    setRows(nextRows);
    setGroups(nextGroups);
  } catch (error) {
    message.error(String(error));
  } finally {
    setLoading(false);
  }
}
```

Add:

```tsx
const groupOptions = useMemo(
  () => groups.map((group) => ({ value: group.id, label: group.name || group.id })),
  [groups],
);
```

- [ ] **Step 6: Add import/export functions**

Add functions inside `ApisPage`:

```tsx
async function openExport(mode: 'api' | 'docs') {
  setExportMode(mode);
  setCheckedApiIds([]);
  setTree(await apiConfigService.tree());
  setExportOpen(true);
}

async function confirmExport() {
  const ids = checkedApiIds.map(String).filter(Boolean);
  if (!ids.length) {
    message.warning('请选择 API');
    return;
  }
  const blob = exportMode === 'api' ? await apiConfigService.exportConfig(ids) : await apiConfigService.exportDocs(ids);
  downloadBlob(blob, exportMode === 'api' ? 'api_config.json' : 'API Doc.md');
  setExportOpen(false);
}

async function importApis(file: File | undefined) {
  if (!file) return;
  await apiConfigService.importConfig(file);
  message.success('导入成功');
  await load();
}

async function importGroups(file: File | undefined) {
  if (!file) return;
  await apiConfigService.importGroups(file);
  message.success('导入成功');
  await load();
}

async function createGroup() {
  const name = newGroupName.trim();
  if (!name) return;
  await groupService.create(name);
  setNewGroupName('');
  await load();
}

async function removeGroup(id: string) {
  await groupService.remove(id);
  await load();
}

async function confirmGroupExport() {
  if (!checkedGroupIds.length) {
    message.warning('请选择分组');
    return;
  }
  const blob = await apiConfigService.exportGroups(checkedGroupIds);
  downloadBlob(blob, 'api_group_config.json');
  setGroupExportOpen(false);
}
```

- [ ] **Step 7: Replace toolbar JSX**

In the toolbar `<Space>`, use:

```tsx
<Select
  allowClear
  className="min-w-40"
  placeholder="分组"
  value={groupId}
  options={groupOptions}
  onChange={setGroupId}
/>
<Input.Search
  allowClear
  addonBefore={
    <Select
      allowClear
      className="w-24"
      placeholder="字段"
      value={field}
      options={[
        { value: 'name', label: '名称' },
        { value: 'path', label: '路径' },
        { value: 'note', label: '备注' },
      ]}
      onChange={setField}
    />
  }
  placeholder="搜索名称 / 路径"
  value={keyword}
  onChange={(event) => setKeyword(event.target.value)}
  onSearch={load}
/>
<Button icon={<ReloadOutlined />} onClick={load}>刷新</Button>
<Button icon={<FolderOutlined />} onClick={() => setGroupModalOpen(true)}>分组</Button>
<Button icon={<FileTextOutlined />} onClick={() => openExport('docs')}>导出文档</Button>
<Button icon={<DownloadOutlined />} onClick={() => openExport('api')}>导出 API</Button>
<Button icon={<UploadOutlined />} onClick={() => apiImportRef.current?.click()}>导入 API</Button>
<Button icon={<DownloadOutlined />} onClick={() => setGroupExportOpen(true)}>导出分组</Button>
<Button icon={<UploadOutlined />} onClick={() => groupImportRef.current?.click()}>导入分组</Button>
<Button type="primary" icon={<PlusOutlined />} onClick={() => navigate('/apis/new')}>创建 API</Button>
<input ref={apiImportRef} type="file" accept=".json" className="hidden" onChange={(event) => void importApis(event.target.files?.[0])} />
<input ref={groupImportRef} type="file" accept=".json" className="hidden" onChange={(event) => void importGroups(event.target.files?.[0])} />
```

- [ ] **Step 8: Add modals**

Before the closing `</div>` of `ApisPage`, add:

```tsx
<Modal title="API 分组" open={groupModalOpen} onCancel={() => setGroupModalOpen(false)} footer={null}>
  <Space.Compact className="mb-4 w-full">
    <Input value={newGroupName} placeholder="新分组名称" onChange={(event) => setNewGroupName(event.target.value)} onPressEnter={createGroup} />
    <Button type="primary" onClick={createGroup}>创建</Button>
  </Space.Compact>
  <Space wrap>
    {groups.map((group) => (
      <Tag key={group.id} closable onClose={(event) => { event.preventDefault(); void removeGroup(group.id!); }}>
        {group.name}
      </Tag>
    ))}
  </Space>
</Modal>

<Modal title={exportMode === 'api' ? '导出 API' : '导出 API 文档'} open={exportOpen} onCancel={() => setExportOpen(false)} onOk={confirmExport}>
  <Tree
    checkable
    treeData={tree.map((node) => ({
      key: node.name,
      title: node.name,
      selectable: false,
      children: (node.children || []).map((api) => ({ key: api.id!, title: `${api.name} ${api.path}` })),
    }))}
    checkedKeys={checkedApiIds}
    onCheck={(keys) => setCheckedApiIds(Array.isArray(keys) ? keys : keys.checked)}
  />
</Modal>

<Modal title="导出 API 分组" open={groupExportOpen} onCancel={() => setGroupExportOpen(false)} onOk={confirmGroupExport}>
  <Select
    mode="multiple"
    className="w-full"
    placeholder="选择分组"
    value={checkedGroupIds}
    options={groupOptions}
    onChange={setCheckedGroupIds}
  />
</Modal>
```

- [ ] **Step 9: Run frontend build**

Run:

```bash
cd frontend
rtk npm run build
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 10: Commit frontend work**

Run:

```bash
rtk git add frontend/src/api/client.ts frontend/src/api/types.ts frontend/src/api/services.ts frontend/src/pages/ApisPage.tsx
rtk git commit -m "feat: add api import export ui"
```

Expected: one commit is created.

---

### Task 3: PostgreSQL Business Datasource Compose and Seeds

**Files:**
- Modify: `docker-compose.yml`
- Create: `docker/postgres/init/001-demo-items.sql`
- Create: `seed_pg_demo_api.sql`
- Modify: `data.db`

- [ ] **Step 1: Add PostgreSQL to Docker Compose**

Replace `docker-compose.yml` with:

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: dbapi_demo
      POSTGRES_USER: dbapi
      POSTGRES_PASSWORD: dbapi_pass
    ports:
      - "15432:5432"
    volumes:
      - pg-data:/var/lib/postgresql/data
      - ./docker/postgres/init:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U dbapi -d dbapi_demo"]
      interval: 5s
      timeout: 3s
      retries: 20

  db-api-rs:
    build:
      context: .
      dockerfile: Dockerfile
    environment:
      RUST_LOG: info
      DB_API_METADATA_URL: sqlite:///data/data.db
    ports:
      - "8520:8520"
    volumes:
      - ./data.db:/data/data.db
    depends_on:
      postgres:
        condition: service_healthy

volumes:
  pg-data:
```

- [ ] **Step 2: Add PostgreSQL business table init SQL**

Create `docker/postgres/init/001-demo-items.sql`:

```sql
CREATE TABLE IF NOT EXISTS demo_items (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  note TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION set_demo_items_updated_at()
RETURNS trigger AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS demo_items_set_updated_at ON demo_items;
CREATE TRIGGER demo_items_set_updated_at
BEFORE UPDATE ON demo_items
FOR EACH ROW
EXECUTE FUNCTION set_demo_items_updated_at();

INSERT INTO demo_items (name, status, note)
VALUES ('Alpha PG', 'active', 'postgres demo item')
ON CONFLICT DO NOTHING;
```

- [ ] **Step 3: Add SQLite metadata seed for PG datasource and API copies**

Create `seed_pg_demo_api.sql` with:

```sql
INSERT OR IGNORE INTO api_group (id, name) VALUES ('pg_crud_group', 'pg crud');

INSERT OR REPLACE INTO datasource (id, name, note, type, url, username, password, driver, table_sql, create_time, update_time)
VALUES (
  'postgres_demo',
  'PostgreSQL 示例库',
  'Docker Compose PostgreSQL business datasource',
  'postgres',
  'postgres://postgres:5432/dbapi_demo',
  'dbapi',
  'dbapi_pass',
  'org.postgresql.Driver',
  NULL,
  datetime('now', 'localtime'),
  datetime('now', 'localtime')
);

DELETE FROM api_sql WHERE api_id IN (
  'pg_demo_item_create', 'pg_demo_item_get', 'pg_demo_item_update', 'pg_demo_item_delete',
  'pg_demo_item_qb_list', 'pg_demo_item_view_sql_list'
);
DELETE FROM api_config WHERE id IN (
  'pg_demo_item_create', 'pg_demo_item_get', 'pg_demo_item_update', 'pg_demo_item_delete',
  'pg_demo_item_qb_list', 'pg_demo_item_view_sql_list'
);

INSERT INTO api_config (id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param)
VALUES
('pg_demo_item_create', '/pg/demo/items/create', 'PG 创建 Demo Item', 'POST name/status/note 创建 PostgreSQL 记录', '[{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_get', '/pg/demo/items/get', 'PG 查询 Demo Item', '按 id 查询 PostgreSQL 单条记录', '[{"name":"id","type":"bigint"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_update', '/pg/demo/items/update', 'PG 更新 Demo Item', '按 id 更新 PostgreSQL name/status/note', '[{"name":"id","type":"bigint"},{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_delete', '/pg/demo/items/delete', 'PG 删除 Demo Item', '按 id 删除 PostgreSQL 记录', '[{"name":"id","type":"bigint"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_qb_list', '/pg/demo/items/qb-list', 'PG Demo Item QueryBuilder List', 'PostgreSQL QueryBuilder 列表接口', '[{"name":"keyword","type":"string"},{"name":"status","type":"string"},{"name":"limit","type":"bigint"},{"name":"offset","type":"bigint"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_view_sql_list', '/pg/demo/items/view-sql-list', 'PG Demo Item View SQL List', 'PostgreSQL View SQL 列表接口', '[{"name":"status","type":"string"}]', 1, 'postgres_demo', 0, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL);

INSERT INTO api_sql (api_id, sql_text, transform_plugin, transform_plugin_params)
VALUES
('pg_demo_item_create', 'INSERT INTO demo_items (name, status, note, created_at, updated_at) VALUES ($name, $status, $note, now(), now())', 'sql', ''),
('pg_demo_item_get', 'SELECT id, name, status, note, created_at, updated_at FROM demo_items WHERE id = $id', 'sql', 'resultType=object'),
('pg_demo_item_update', 'UPDATE demo_items SET name = $name, status = $status, note = $note, updated_at = now() WHERE id = $id', 'sql', ''),
('pg_demo_item_delete', 'DELETE FROM demo_items WHERE id = $id', 'sql', ''),
('pg_demo_item_qb_list', '{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true}', 'queryBuilder', 'resultType=page'),
('pg_demo_item_view_sql_list', 'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]', 'viewSql', 'resultType=page'),
('pg_demo_item_view_sql_list', 'select count(*) as total from demo_items a where a.status = $status', 'viewSqlCount', '');
```

- [ ] **Step 4: Apply PG metadata seed to tracked SQLite database**

Run:

```bash
rtk sqlite3 data.db < seed_pg_demo_api.sql
```

Expected: no output and exit code 0.

Verify:

```bash
rtk sqlite3 data.db "select id,name,type,url from datasource where id='postgres_demo'; select id,name from api_group where id='pg_crud_group'; select id,path,datasource_id,group_id from api_config where group_id='pg_crud_group' order by id;"
```

Expected: one `postgres_demo` datasource, one `pg_crud_group`, and six PG API rows.

- [ ] **Step 5: Commit compose and seeds**

Run:

```bash
rtk git add docker-compose.yml docker/postgres/init/001-demo-items.sql seed_pg_demo_api.sql data.db
rtk git commit -m "feat: add postgres demo datasource seed"
```

Expected: one commit is created. Do not add `data.db-shm` or `data.db-wal`.

---

### Task 4: Full Verification and Local Smoke Test

**Files:**
- No source edits expected.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
rtk cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run frontend tests and build**

Run:

```bash
cd frontend
rtk npm test -- --run
rtk npm run build
```

Expected: Vitest and Vite build pass.

- [ ] **Step 3: Build and start Docker Compose**

Run:

```bash
rtk docker compose up -d --build
```

Expected: `db-api-rs` and `postgres` containers are running.

- [ ] **Step 4: Verify app health**

Run:

```bash
rtk curl -s http://127.0.0.1:8520/health
```

Expected:

```text
OK
```

- [ ] **Step 5: Verify PostgreSQL API execution**

Run:

```bash
rtk curl -s -X POST http://127.0.0.1:8520/api/pg/demo/items/qb-list \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'status=active&keyword=&limit=20&offset=0'
```

Expected: JSON envelope with `success:true`, `data.list`, and `data.total`.

Run:

```bash
rtk curl -s -X POST http://127.0.0.1:8520/api/pg/demo/items/view-sql-list \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'columns=a.id,a.name,a.status,a.note,a.created_at,a.updated_at' \
  --data 'order_by=a.id&status=active&limit=20&offset=0'
```

Expected: JSON envelope with `success:true`, `data.list`, and `data.total`.

- [ ] **Step 6: Verify import/export endpoints directly**

Export PG APIs:

```bash
rtk curl -s -X POST 'http://127.0.0.1:8520/apiConfig/downloadConfig?ids=pg_demo_item_qb_list,pg_demo_item_view_sql_list' > /tmp/pg_api_config.json
rtk jq '.api | length, .sql | length' /tmp/pg_api_config.json
```

Expected:

```text
2
3
```

Export PG group:

```bash
rtk curl -s -X POST 'http://127.0.0.1:8520/apiConfig/downloadGroupConfig?ids=pg_crud_group' > /tmp/pg_group_config.json
rtk jq '.[0].name' /tmp/pg_group_config.json
```

Expected:

```text
"pg crud"
```

- [ ] **Step 7: Check git status**

Run:

```bash
rtk git status --short --branch
```

Expected: no uncommitted tracked source changes except ignored/generated runtime files. If `data.db-shm` or `data.db-wal` appear, leave them uncommitted.

- [ ] **Step 8: Push main**

Run:

```bash
rtk git push origin main
```

Expected: push succeeds to `origin/main`.
