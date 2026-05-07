PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS demo_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  note TEXT,
  created_at TEXT,
  updated_at TEXT
);

CREATE TRIGGER IF NOT EXISTS demo_items_insert_timestamps
AFTER INSERT ON demo_items
FOR EACH ROW
WHEN NEW.created_at IS NULL OR NEW.updated_at IS NULL
BEGIN
  UPDATE demo_items
  SET created_at = COALESCE(created_at, datetime('now', 'localtime')),
      updated_at = COALESCE(updated_at, datetime('now', 'localtime'))
  WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS demo_items_update_timestamp
AFTER UPDATE OF name, status, note ON demo_items
FOR EACH ROW
BEGIN
  UPDATE demo_items
  SET updated_at = datetime('now', 'localtime')
  WHERE id = NEW.id;
END;

INSERT OR IGNORE INTO api_group (id, name) VALUES ('demo_crud_group', 'Demo CRUD 示例');

INSERT OR REPLACE INTO datasource (id, name, note, type, url, username, password, driver, table_sql, create_time, update_time)
VALUES (
  'local_sqlite_demo',
  '当前 SQLite 示例库',
  '使用当前 data.db 作为示例业务库',
  'sqlite',
  'sqlite://data.db',
  '',
  '',
  'org.sqlite.JDBC',
  NULL,
  datetime('now', 'localtime'),
  datetime('now', 'localtime')
);

DELETE FROM api_sql WHERE api_id IN (
  'demo_item_create', 'demo_item_get', 'demo_item_update', 'demo_item_delete',
  'demo_item_list', 'demo_item_qb_list', 'demo_item_filter', 'demo_item_count'
);
DELETE FROM api_config WHERE id IN (
  'demo_item_create', 'demo_item_get', 'demo_item_update', 'demo_item_delete',
  'demo_item_list', 'demo_item_qb_list', 'demo_item_filter', 'demo_item_count'
);

INSERT INTO api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param)
VALUES
('demo_item_create', 'demo/items/create', 'POST', '创建 Demo Item', 'POST name/status/note 创建记录', '[{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'local_sqlite_demo', 0, 'demo_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('demo_item_get', 'demo/items/get', 'GET', '查询 Demo Item', '按 id 查询单条记录', '[{"name":"id","type":"bigint"}]', 1, 'local_sqlite_demo', 0, 'demo_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('demo_item_update', 'demo/items/update', 'PATCH', '更新 Demo Item', '按 id 更新 name/status/note', '[{"name":"id","type":"bigint"},{"name":"name","type":"string"},{"name":"status","type":"string"},{"name":"note","type":"string"}]', 1, 'local_sqlite_demo', 0, 'demo_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('demo_item_delete', 'demo/items/delete', 'DELETE', '删除 Demo Item', '按 id 删除记录', '[{"name":"id","type":"bigint"}]', 1, 'local_sqlite_demo', 0, 'demo_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL),
('demo_item_qb_list', '/demo/items/qb-list', 'GET', 'Demo Item QueryBuilder List', 'QueryBuilder 列表接口，支持 keyword/status 过滤、limit/offset 分页，并返回 total', '[{"name":"keyword","type":"string"},{"name":"status","type":"string"},{"name":"limit","type":"number"},{"name":"offset","type":"number"}]', 1, 'local_sqlite_demo', 0, 'demo_crud_group', NULL, NULL, datetime('now', 'localtime'), datetime('now', 'localtime'), 'application/x-www-form-urlencoded', 0, NULL);

INSERT INTO api_sql (api_id, sql_text, transform_plugin, transform_plugin_params)
VALUES
('demo_item_create', 'INSERT INTO demo_items (name, status, note) VALUES ($name, $status, $note)', NULL, NULL),
('demo_item_get', 'SELECT id, name, status, note, created_at, updated_at FROM demo_items WHERE id = $id', NULL, 'resultType=object'),
('demo_item_update', 'UPDATE demo_items SET name = $name, status = $status, note = $note WHERE id = $id', NULL, NULL),
('demo_item_delete', 'DELETE FROM demo_items WHERE id = $id', NULL, NULL),
('demo_item_qb_list', '{"type":"queryBuilder","table":"demo_items","select":["id","name","status","note","created_at","updated_at"],"rules":{"combinator":"and","rules":[{"field":"name","operator":"contains","valueSource":"param","value":{"param":"keyword"},"skipWhen":["missing","empty_string"]},{"field":"status","operator":"=","valueSource":"param","value":{"param":"status"},"skipWhen":["missing","empty_string"]}]},"orderBy":[{"field":"id","direction":"desc"}],"limit":{"param":"limit","default":20,"max":100},"offset":{"param":"offset","default":0},"count":true}', 'queryBuilder', 'resultType=page');
