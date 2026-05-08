INSERT OR IGNORE INTO api_group (id, name) VALUES ('mysql_crud_group', 'mysql crud');

INSERT OR REPLACE INTO datasource (id, name, note, type, url, username, password, driver, table_sql, create_time, update_time)
VALUES (
  'mysql_demo',
  'MySQL 示例库',
  'Docker Compose MySQL business datasource',
  'mysql',
  'mysql://mysql:3306/dbapi_demo',
  'dbapi',
  'dbapi_pass',
  'com.mysql.cj.jdbc.Driver',
  NULL,
  datetime('now', 'localtime'),
  datetime('now', 'localtime')
);

DELETE FROM api_sql WHERE api_id IN (
  'mysql_demo_item_create', 'mysql_demo_item_get', 'mysql_demo_item_update', 'mysql_demo_item_delete',
  'mysql_demo_item_qb_list', 'mysql_demo_item_view_sql_list'
);
DELETE FROM api_config WHERE id IN (
  'mysql_demo_item_create', 'mysql_demo_item_get', 'mysql_demo_item_update', 'mysql_demo_item_delete',
  'mysql_demo_item_qb_list', 'mysql_demo_item_view_sql_list'
);

INSERT INTO api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param)
VALUES
('mysql_demo_item_create', '/mysql/demo/items/create', 'POST', 'MySQL 创建 Demo Item', 'POST name/status/note 创建 MySQL 记录', '[{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_get', '/mysql/demo/items/get', 'GET', 'MySQL 查询 Demo Item', '按 id 查询 MySQL 单条记录', '[{"name":"id","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_update', '/mysql/demo/items/update', 'PATCH', 'MySQL 更新 Demo Item', '按 id 更新 MySQL name/status/note', '[{"name":"id","type":"bigint"},{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_delete', '/mysql/demo/items/delete', 'DELETE', 'MySQL 删除 Demo Item', '按 id 删除 MySQL 记录', '[{"name":"id","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_qb_list', '/mysql/demo/items/qb-list', 'GET', 'MySQL Demo Item QueryBuilder List', 'MySQL QueryBuilder 列表接口', '[{"name":"keyword","type":"string"},{"name":"status","type":"string"},{"name":"limit","type":"bigint"},{"name":"offset","type":"bigint"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('mysql_demo_item_view_sql_list', '/mysql/demo/items/view-sql-list', 'GET', 'MySQL Demo Item View SQL List', 'MySQL View SQL 列表接口', '[{"name":"status","type":"string"}]', 1, 'mysql_demo', 1, 'mysql_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL);

INSERT INTO api_sql (api_id, sql_text, transform_plugin, transform_plugin_params)
VALUES
('mysql_demo_item_create', 'INSERT INTO demo_items (name, status, note) VALUES ($name, $status, $note)', 'sql', ''),
('mysql_demo_item_get', 'SELECT id, name, status, note, created_at, updated_at FROM demo_items WHERE id = $id', 'sql', 'resultType=object'),
('mysql_demo_item_update', 'UPDATE demo_items SET name = $name, status = $status, note = $note, updated_at = current_timestamp WHERE id = $id', 'sql', ''),
('mysql_demo_item_delete', 'DELETE FROM demo_items WHERE id = $id', 'sql', ''),
('mysql_demo_item_qb_list', '{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true}', 'queryBuilder', 'resultType=page'),
('mysql_demo_item_view_sql_list', 'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]', 'viewSql', 'resultType=page'),
('mysql_demo_item_view_sql_list', 'select count(*) as total from demo_items a where a.status = $status', 'viewSqlCount', '');
