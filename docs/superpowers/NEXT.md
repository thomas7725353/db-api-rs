# DBAPI Rust 下一步工作

更新时间：2026-05-05

## 当前已完成

- Rust standalone runtime 已替代 Java 版核心后端，当前服务端口为 `8520`。
- React + Ant Design + Tailwind 前端已放在 `runtime-rust/frontend`，构建产物输出到 `runtime-rust/static`。
- QueryBuilder 后端已接入 `SeaQuery`，支持基础规则查询、分页、count 和 `/api/{path}` 动态执行。
- Token 鉴权和访问日志已移植到 Rust，Demo CRUD API 已能用 token 调用。
- `demo/items/get` 已通过 `api_sql.transform_plugin_params = resultType=object` 返回单个对象，不再返回数组。
- GitHub public 仓库已创建并推送：`https://github.com/thomas7725353/db-api`。

## 下次优先处理

1. 完善 QueryBuilder 前端体验。
   - 当前已接入真实 `react-querybuilder` 组件，但需要继续优化字段来源、操作符中文化、值类型编辑器、数组/in 输入、日期/数字输入。
   - JSON/SQL 预览应保持为高级折叠区，不能回退成纯 JSON 编辑框。

2. 实现 QueryBuilder 参数模板。
   - 支持规则值绑定请求参数，例如 `valueSource = param` 或项目自定义的 `{ "param": "statusList", "default": [...] }`。
   - 明确区分缺失参数、`null`、空字符串、空数组、`0`、`false`。
   - 前端需要能选择“固定值”或“请求参数”，否则无法做真正可复用 API 模板。

3. 完善 QueryBuilder API 编辑保存。
   - API 编辑页保存 `queryBuilder` 时要同步生成/维护 `params` 元数据。
   - 对 `list`、`get`、`count`、`object` 等返回模式提供 UI 选项，底层继续用 `transform_plugin_params` 存储。

4. 补齐可视化查询的真实数据源字段。
   - 根据选定 datasource 和 table 拉取字段列表。
   - 字段只能从真实 schema 或白名单中选择，避免任意 identifier 注入。
   - 支持表名、字段名、排序字段的安全校验。

5. 继续强化监控页。
   - 访问日志已经写入，但图表聚合和前端展示还要继续对齐 3.3.0 行为。
   - 验证成功/失败趋势、Top API、Top client/app、Top IP 的数据是否完整。

6. 清理公开仓库风险。
   - 当前仓库是 public，后续应评估是否继续提交 `data.db`、`data.db-wal`、`data.db-shm`。
   - 如果保留 demo 数据库，需要确保没有真实 secret/token。
   - 建议后续改成 seed SQL + 示例 DB，而不是提交运行态 WAL 文件。

## 已验证命令

```bash
rtk npm run build
rtk cargo test
rtk curl -sS -H "Authorization: <TOKEN>" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  -X POST --data-urlencode 'id=1' \
  'http://127.0.0.1:8520/api/demo/items/get'
```

`demo/items/get` 当前期望返回：

```json
{
  "success": true,
  "msg": "接口访问成功",
  "data": {
    "id": 1
  }
}
```

## 注意事项

- 本地 `origin` 仍指向 Gitee：`https://gitee.com/freakchicken/db-api.git`。
- GitHub remote 名为 `github`：`https://github.com/thomas7725353/db-api.git`。
- 推送 GitHub 如果遇到 HTTP 400，已设置本仓库 `http.version = HTTP/1.1` 和 `http.postBuffer = 524288000` 后可成功推送。
