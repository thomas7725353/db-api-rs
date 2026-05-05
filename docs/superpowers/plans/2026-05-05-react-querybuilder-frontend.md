# React QueryBuilder Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the opaque Vue static bundle with a maintainable React/Vite frontend and make QueryBuilder the primary API editing path.

**Architecture:** Create `runtime-rust/frontend` as the frontend source tree. Build it into `runtime-rust/static` so the existing Axum server continues to serve a single app on port `8520`. Use existing Rust endpoints for first-pass compatibility; store QueryBuilder definitions in existing `api_sql.sql_text` with `transform_plugin = "queryBuilder"`.

**Tech Stack:** React 19, Vite, TypeScript, Ant Design, react-querybuilder, @react-querybuilder/antd, Tailwind CSS.

---

### Task 1: Scaffold React Frontend

**Files:**
- Create: `runtime-rust/frontend/package.json`
- Create: `runtime-rust/frontend/index.html`
- Create: `runtime-rust/frontend/tsconfig.json`
- Create: `runtime-rust/frontend/vite.config.ts`
- Create: `runtime-rust/frontend/src/main.tsx`
- Create: `runtime-rust/frontend/src/App.tsx`
- Create: `runtime-rust/frontend/src/styles.css`

- [x] Add Vite/React/TypeScript dependencies.
- [x] Configure Vite output to `../static`.
- [x] Add Ant Design and Tailwind CSS imports.
- [x] Verify `npm install` and `npm run build`.

### Task 2: Add Shared API Client And Types

**Files:**
- Create: `runtime-rust/frontend/src/api/client.ts`
- Create: `runtime-rust/frontend/src/api/types.ts`
- Create: `runtime-rust/frontend/src/api/services.ts`

- [x] Implement `apiGet`, `apiPost`, and `apiRequest`.
- [x] Normalize endpoints that return raw arrays, raw objects, or `{success,msg,data}`.
- [x] Add TypeScript types for datasource, API config, API SQL, group, app info, and access log.
- [x] Add service functions for datasource, API config, groups, apps, monitor, and system endpoints.

### Task 3: Add Layout And Routing

**Files:**
- Create: `runtime-rust/frontend/src/layout/AppLayout.tsx`
- Create: `runtime-rust/frontend/src/pages/DatasourcesPage.tsx`
- Create: `runtime-rust/frontend/src/pages/ApisPage.tsx`
- Create: `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`
- Create: `runtime-rust/frontend/src/pages/ApiRequestPage.tsx`
- Create: `runtime-rust/frontend/src/pages/TokensPage.tsx`
- Create: `runtime-rust/frontend/src/pages/MonitorPage.tsx`

- [x] Add top navigation for Datasources, APIs, Tokens, Monitor.
- [x] Show version and standalone mode in the header.
- [x] Add route fallback to APIs page.

### Task 4: Implement Datasource Page

**Files:**
- Modify: `runtime-rust/frontend/src/pages/DatasourcesPage.tsx`

- [x] Load datasource list.
- [x] Add create/edit modal.
- [x] Support SQLite/MySQL/Postgres fields.
- [x] Call connect test.
- [x] Delete datasource and refresh list.

### Task 5: Implement API List And Editor

**Files:**
- Modify: `runtime-rust/frontend/src/pages/ApisPage.tsx`
- Modify: `runtime-rust/frontend/src/pages/ApiEditorPage.tsx`
- Create: `runtime-rust/frontend/src/components/QueryBuilderEditor.tsx`

- [x] Load API configs and groups.
- [x] Add online/offline/delete actions.
- [x] Add SQL mode editor textarea.
- [x] Add QueryBuilder mode with `react-querybuilder` and AntD controls.
- [x] Serialize QueryBuilder DSL into `api_sql.sql_text`.
- [x] Save API config through `/apiConfig/add` and `/apiConfig/update`.

### Task 6: Implement Request, Token, And Monitor Pages

**Files:**
- Modify: `runtime-rust/frontend/src/pages/ApiRequestPage.tsx`
- Modify: `runtime-rust/frontend/src/pages/TokensPage.tsx`
- Modify: `runtime-rust/frontend/src/pages/MonitorPage.tsx`

- [x] Request page loads API detail and builds parameter JSON.
- [x] Token page creates app, lists apps, authorizes groups, and generates token URL hints.
- [x] Monitor page loads access logs and summary endpoints.

### Task 7: Build Integration

**Files:**
- Modify: `runtime-rust/Dockerfile`
- Modify: `docker-compose.yml` only if needed.

- [x] Add Node build stage to Dockerfile.
- [x] Copy frontend build to runtime image static directory.
- [x] Run `npm run build`.
- [x] Run `cargo check`.
- [x] Start Rust server and verify `http://127.0.0.1:8520`.
