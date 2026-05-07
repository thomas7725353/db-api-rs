# db-api-rs 使用文档

`db-api-rs` 是一个 Rust 运行时和 React 管理界面，用来把数据库查询快速发布成 HTTP API。它适合内部工具、管理后台、数据服务原型和需要把 SQLite、MySQL、PostgreSQL 表或 SQL 查询包装成接口的场景。

## 快速启动

使用 Docker Compose 启动服务：

```bash
docker compose up -d --build
```

默认服务地址：

```text
http://127.0.0.1:8520
```

健康检查：

```bash
curl http://127.0.0.1:8520/health
```

Compose 会把仓库根目录的 `data.db` 挂载到容器内作为元数据和本地演示数据库：

```yaml
volumes:
  - ./data.db:/data/data.db
```

## 核心能力

- 在 Web UI 里管理数据源。
- 把 API 发布到 `/api/{path}`。
- 支持公开 API 和 token 保护 API。
- 记录 API 访问日志。
- 支持 SQLite、MySQL、PostgreSQL 数据源。
- 支持 `GET`、`POST`、`PUT`、`PATCH`、`DELETE` 等 HTTP 方法配置。

## API 创建方式

### QueryBuilder

QueryBuilder 使用结构化查询 DSL 创建常见的列表、筛选、分页和计数接口。它适合表级 CRUD、列表页、管理后台查询这类规则清晰的接口。

### SQL

SQL 模式适合手写固定 SQL。参数使用 `$name` 这种命名参数：

```sql
select id, name from demo_items where status = $status
```

不要使用 `$1` 这类位置参数。

### View SQL

View SQL 使用 MiniJinja 模板生成安全的 SQL 结构片段，适合复杂 join、动态列、排序、limit 和 offset。

示例：

```sql
select [[ columns | ident_list ]]
from demo_items
where status = $status
order by [[ order_by | ident ]] desc
limit [[ limit | int(default=10,max=1000) ]]
offset [[ offset | int(default=0) ]]
```

值参数仍然用 `$status` 绑定；结构参数只能通过受限过滤器生成安全片段。

## 本地开发

启动 Rust 后端：

```bash
cargo run
```

启动前端开发服务器：

```bash
cd frontend
npm install
npm run dev
```

构建前端静态资源：

```bash
cd frontend
npm run build
```

运行检查：

```bash
cargo test

cd frontend
npm test -- --run
npm run build
```

## Bundle 工作流

推荐用 bundle 工作流批量生成、审查、验证和导入 API 配置。生成目录通常包含：

- `dbapi_manifest.json`
- `api_group_config.json`
- `api_config.json`
- `curl.md`
- `VERIFY.md`

从表生成 CRUD/list/view bundle：

```bash
cargo run -- bundle draft-table \
  --base-url http://127.0.0.1:8520 \
  --datasource-id postgres_demo \
  --table demo_items \
  --resource-path pg/demo/items \
  --group-id demo_items_group \
  --group-name "PG Demo Items" \
  --out target/dbapi-bundles/demo_items
```

验证：

```bash
cargo run -- bundle validate \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items
```

确认文件无误后导入：

```bash
cargo run -- bundle apply \
  --base-url http://127.0.0.1:8520 \
  --dir target/dbapi-bundles/demo_items \
  --allow-write
```

从 SQL 生成单个 API bundle：

```bash
cargo run -- bundle draft-sql \
  --datasource-id postgres_demo \
  --resource-path demo/items/custom-search \
  --api-id demo_items_custom_search \
  --api-name "Demo Items Custom Search" \
  --group-id demo_items_group \
  --group-name "PG Demo Items" \
  --sql 'select id, name from demo_items where status = $status' \
  --engine sql \
  --out target/dbapi-bundles/demo_items_custom_search
```

`--engine` 可用值：

- `sql`
- `viewSql`

## MCP Sidecar

Docker Compose 会同时启动 MCP HTTP sidecar：

```text
http://127.0.0.1:8521/mcp
```

MCP 工具：

- `list_datasources`
- `inspect_table_schema`
- `draft_table_crud_bundle`
- `draft_sql_api_bundle`
- `validate_api_bundle`
- `apply_api_config_bundle`

默认 sidecar 是只读、草稿和验证模式。写入需要启动服务时加 `--allow-write`，并且调用 apply 工具时显式传 `allowWrite=true`。

## Agent Skills

仓库提供 repo-local skills，适合让 Codex、Claude、Cursor 等 agent 复用固定工作流：

- `skills/dbapi-generate-table-apis`
- `skills/dbapi-generate-sql-api`
- `skills/dbapi-apply-api-bundle`
- `skills/dbapi-token-workflow`
- `skills/dbapi-export-import-workflow`

常用触发示例：

```text
use skill dbapi-generate-table-apis 给 postgres_demo.demo_items 生成 API，
resource_path=demo/items，group_id=demo_items_group，group_name=PG Demo Items
```

## 项目地址

GitHub 仓库：

```text
https://github.com/thomas7725353/db-api-rs
```
