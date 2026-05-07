• 现在推荐按这个顺序用：Skills 让 AI 生成 bundle，CLI 做本地验证和导入，MCP 给 Cursor/Codex/Claude 这类 agent 远程调用同一套能力。

MCP
启动：

rtk docker compose up -d --build

MCP 地址：

http://127.0.0.1:8521/mcp

给 Cursor/Codex/Claude 配 MCP 时填这个 HTTP endpoint。直接浏览器/curl 打 /mcp 如果看到 406 Not Acceptable 或要求 text/event-stream，不一定是
坏了，说明它是 MCP streamable HTTP endpoint，需要 MCP client 连。

MCP 工具目前是这些：

list_datasources
inspect_table_schema
draft_table_crud_bundle
draft_sql_api_bundle
validate_api_bundle
apply_api_config_bundle

默认 compose 里的 MCP sidecar 是只读/草稿/验证模式。要允许直接写入 db-api-rs，需要启动时加：

rtk cargo run -- mcp \
 --transport http \
 --listen 0.0.0.0:8521 \
 --base-url http://127.0.0.1:8520 \
 --allow-write

即使服务端开了 --allow-write，调用 apply 工具时也还要显式传 allowWrite=true。

CLI
先看命令：

rtk cargo run -- --help
rtk cargo run -- bundle --help
rtk cargo run -- mcp --help

从表生成 CRUD/list/table/view bundle：

rtk cargo run -- bundle draft-table \
 --base-url http://127.0.0.1:8520 \
 --datasource-id postgres_demo \
 --table demo_items \
 --resource-path demo/items \
 --group-id demo_items_group \
 --group-name "PG Demo Items" \
 --out target/dbapi-bundles/demo_items

生成的接口默认是：

POST demo/items/create SQL
GET demo/items/get SQL
PATCH demo/items/update SQL
DELETE demo/items/delete SQL
GET demo/items/qb-list QueryBuilder page
GET demo/items/table QueryBuilder page
GET demo/items/view-sql-list View SQL page

生成文件：

dbapi_manifest.json
api_group_config.json
api_config.json
curl.md
VERIFY.md

验证：

rtk cargo run -- bundle validate \
 --base-url http://127.0.0.1:8520 \
 --dir target/dbapi-bundles/demo_items

确认文件没问题后导入：

rtk cargo run -- bundle apply \
 --base-url http://127.0.0.1:8520 \
 --dir target/dbapi-bundles/demo_items \
 --allow-write

从一段 SQL 生成 API：

rtk cargo run -- bundle draft-sql \
 --datasource-id postgres_demo \
 --resource-path demo/items/custom-search \
 --api-id demo_items_custom_search \
 --api-name "Demo Items Custom Search" \
 --group-id demo_items_group \
 --group-name "PG Demo Items" \
 --sql 'select id, name from demo_items where status = $status' \
 --engine sql \
 --out target/dbapi-bundles/demo_items_custom_search

--engine 可用：

sql
viewSql

注意：参数用 $status 这种命名参数；不要用 $1。

Skills
repo 里现在有这些 agent skill：

skills/dbapi-generate-table-apis/SKILL.md
skills/dbapi-generate-sql-api/SKILL.md
skills/dbapi-apply-api-bundle/SKILL.md
skills/dbapi-token-workflow/SKILL.md
skills/dbapi-export-import-workflow/SKILL.md

给 agent 的用法可以这样说：

use skill dbapi-generate-table-apis 给 postgres_demo.demo_items 生成 API，
resource_path=demo/items，group_id=demo_items_group，group_name=PG Demo Items

use skill dbapi-generate-sql-api 根据这段 SQL 生成 API bundle:
select id, name from demo_items where status = $status

use skill dbapi-apply-api-bundle validate 这个目录；
确认后 apply

use skill dbapi-token-workflow 给这个 API group 创建 token，并生成 curl 验证

use skill dbapi-export-import-workflow 导出/导入这个 API group

旧的 skills/dbapi-demo-crud 只适合历史 demo，不建议新流程继续用。

完整说明已经放在 README.md:23，重点看 AI and Agent Workflows、DBAPI Bundle Workflow、MCP Sidecar 这几段。
