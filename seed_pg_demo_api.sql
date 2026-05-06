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
('pg_demo_item_create', '/pg/demo/items/create', 'PG 创建 Demo Item', 'POST name/status/note 创建 PostgreSQL 记录', '[{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_get', '/pg/demo/items/get', 'PG 查询 Demo Item', '按 id 查询 PostgreSQL 单条记录', '[{"name":"id","type":"bigint"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_update', '/pg/demo/items/update', 'PG 更新 Demo Item', '按 id 更新 PostgreSQL name/status/note', '[{"name":"id","type":"bigint"},{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_delete', '/pg/demo/items/delete', 'PG 删除 Demo Item', '按 id 删除 PostgreSQL 记录', '[{"name":"id","type":"bigint"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_qb_list', '/pg/demo/items/qb-list', 'PG Demo Item QueryBuilder List', 'PostgreSQL QueryBuilder 列表接口', '[{"name":"keyword","type":"string"},{"name":"status","type":"string"},{"name":"limit","type":"bigint"},{"name":"offset","type":"bigint"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('pg_demo_item_view_sql_list', '/pg/demo/items/view-sql-list', 'PG Demo Item View SQL List', 'PostgreSQL View SQL 列表接口', '[{"name":"status","type":"string"}]', 1, 'postgres_demo', 1, 'pg_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL);

INSERT INTO api_sql (api_id, sql_text, transform_plugin, transform_plugin_params)
VALUES
('pg_demo_item_create', 'INSERT INTO demo_items (name, status, note, created_at, updated_at) VALUES ($name, $status, $note, now(), now())', 'sql', ''),
('pg_demo_item_get', 'SELECT id, name, status, note, created_at, updated_at FROM demo_items WHERE id = $id', 'sql', 'resultType=object'),
('pg_demo_item_update', 'UPDATE demo_items SET name = $name, status = $status, note = $note, updated_at = now() WHERE id = $id', 'sql', ''),
('pg_demo_item_delete', 'DELETE FROM demo_items WHERE id = $id', 'sql', ''),
('pg_demo_item_qb_list', '{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true}', 'queryBuilder', 'resultType=page'),
('pg_demo_item_view_sql_list', 'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]', 'viewSql', 'resultType=page'),
('pg_demo_item_view_sql_list', 'select count(*) as total from demo_items a where a.status = $status', 'viewSqlCount', '');
