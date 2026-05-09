use crate::db::{self, DbConn};
use crate::model::{
    AccessLog, ApiAlarm, ApiConfig, ApiConfigExport, ApiGroup, ApiSql, AppInfo, DataSource, User,
};
use chrono::Local;
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, FromQueryResult, QueryResult, TransactionTrait,
};
use sea_query::Value;
use serde::Serialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub async fn init_repository(url: &str) -> anyhow::Result<DbConn> {
    let db = db::connect_metadata(url).await?;
    ensure_metadata_tables(&db).await?;
    ensure_api_config_method_column(&db).await?;
    ensure_default_admin_user(&db).await?;
    Ok(db)
}

async fn ensure_metadata_tables(db: &DbConn) -> anyhow::Result<()> {
    db::execute(
        db,
        "create table if not exists user (id integer not null primary key autoincrement, username text unique, password text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists datasource (id text not null primary key, name text, note text, type text, url text, username text, password text, driver text, table_sql text, create_time text, update_time text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists api_group (id text not null primary key, name text not null unique)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists api_config (id text not null primary key, path text unique, method text default 'POST', name text, note text, params text, status integer, datasource_id text, previlege integer, group_id text, cache_plugin text, cache_plugin_params text, create_time text, update_time text, content_type text, open_trans integer, json_param text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists api_sql (id integer not null primary key autoincrement, api_id text, sql_text text, transform_plugin text, transform_plugin_params text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists api_alarm (api_id text, alarm_plugin text, alarm_plugin_param text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists access_log (id text primary key, url text, status integer, duration integer, timestamp integer, ip text, app_id text, api_id text, error text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists app_info (id text not null primary key, name text, note text, secret text, expire_desc text, expire_duration text, token text, expire_at text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists api_auth (id integer primary key autoincrement, app_id text, group_id text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists firewall (status text, mode text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists ip_rules (type text, ip text)",
        vec![],
    )
    .await?;
    db::execute(
        db,
        "create table if not exists token (id integer not null primary key autoincrement, token text, note text, expire integer, create_time integer)",
        vec![],
    )
    .await?;
    Ok(())
}

async fn ensure_api_config_method_column(db: &DbConn) -> anyhow::Result<()> {
    let _ = db::execute(
        db,
        "alter table api_config add column method text default 'POST'",
        vec![],
    )
    .await;
    Ok(())
}

async fn ensure_default_admin_user(db: &DbConn) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert or ignore into user (username, password) values ('admin', 'admin')",
        vec![],
    )
    .await?;
    Ok(())
}

pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn v(value: impl Serialize) -> Value {
    db::json_to_db_value(serde_json::json!(value))
}

fn typed_json_rows<T>(rows: Vec<QueryResult>) -> anyhow::Result<Vec<JsonValue>>
where
    T: FromQueryResult + Serialize,
{
    rows.iter()
        .map(|row| {
            let value = T::from_query_result(row, "")?;
            serde_json::to_value(value).map_err(Into::into)
        })
        .collect()
}

async fn count_first(db: &DbConn, sql: &str, args: Vec<Value>) -> i64 {
    #[derive(FromQueryResult)]
    struct CountRow {
        count: i64,
    }

    let Ok(rows) = db.conn.query_all(db.statement(sql, args)).await else {
        return 0;
    };
    rows.first()
        .and_then(|row| CountRow::from_query_result(row, "").ok())
        .map(|row| row.count)
        .unwrap_or(0)
}

const DATASOURCE_COLUMNS: &str =
    "id, name, note, type, url, username, password, driver, table_sql, create_time, update_time";
const API_COLUMNS: &str = "id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param";
const APP_COLUMNS: &str = "id, name, note, secret, expire_desc, expire_duration, token, expire_at";

pub async fn select_all_datasources(db: &DbConn) -> anyhow::Result<Vec<DataSource>> {
    db::query_as(
        db,
        &format!("select {DATASOURCE_COLUMNS} from datasource order by update_time desc"),
        vec![],
    )
    .await
}

pub async fn select_datasource_by_id(db: &DbConn, id: &str) -> anyhow::Result<Option<DataSource>> {
    db::query_one_as(
        db,
        &format!("select {DATASOURCE_COLUMNS} from datasource where id = ?"),
        vec![v(id)],
    )
    .await
}

pub async fn insert_datasource(db: &DbConn, ds: &DataSource) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert into datasource (id, name, note, type, url, username, password, driver, table_sql, create_time, update_time) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v(&ds.id),
            v(&ds.name),
            v(&ds.note),
            v(&ds.db_type),
            v(&ds.url),
            v(&ds.username),
            v(&ds.password),
            v(&ds.driver),
            v(&ds.table_sql),
            v(&ds.create_time),
            v(&ds.update_time),
        ],
    )
    .await?;
    Ok(())
}

pub async fn update_datasource(db: &DbConn, ds: &DataSource) -> anyhow::Result<()> {
    db::execute(
        db,
        "update datasource set name = ?, note = ?, type = ?, url = ?, username = ?, password = ?, driver = ?, table_sql = ?, update_time = ? where id = ?",
        vec![
            v(&ds.name),
            v(&ds.note),
            v(&ds.db_type),
            v(&ds.url),
            v(&ds.username),
            v(&ds.password),
            v(&ds.driver),
            v(&ds.table_sql),
            v(&ds.update_time),
            v(&ds.id),
        ],
    )
    .await?;
    Ok(())
}

pub async fn delete_datasource(db: &DbConn, id: &str) -> anyhow::Result<()> {
    db::execute(db, "delete from datasource where id = ?", vec![v(id)]).await?;
    Ok(())
}

pub async fn count_api_by_datasource(db: &DbConn, datasource_id: &str) -> i64 {
    count_first(
        db,
        "select count(1) as count from api_config where datasource_id = ?",
        vec![v(datasource_id)],
    )
    .await
}

pub async fn select_all_api_configs(db: &DbConn) -> anyhow::Result<Vec<ApiConfig>> {
    let mut configs = db::query_as(
        db,
        &format!("select {API_COLUMNS} from api_config order by update_time desc"),
        vec![],
    )
    .await?;
    fill_api_children_for_configs(db, &mut configs).await?;
    Ok(configs)
}

pub async fn export_api_configs(db: &DbConn, ids: &[String]) -> anyhow::Result<ApiConfigExport> {
    let mut api = Vec::new();
    let mut sql = Vec::new();
    for id in ids {
        if let Some(mut config) = select_api_by_id(db, id).await? {
            let sql_rows = select_api_sqls(db, id).await?;
            sql.extend(sql_rows);
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
    let txn = db.conn.begin().await?;
    for group in groups {
        execute_tx(
            db,
            &txn,
            "insert into api_group (id, name) values (?, ?)",
            vec![v(&group.id), v(&group.name)],
        )
        .await?;
    }
    txn.commit().await?;
    Ok(())
}

pub async fn import_api_configs(db: &DbConn, bundle: &ApiConfigExport) -> anyhow::Result<()> {
    validate_import_api_configs(db, bundle).await?;
    let txn = db.conn.begin().await?;
    for config in &bundle.api {
        execute_tx(
            db,
            &txn,
            "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                v(&config.id),
                v(&config.path),
                v(&config.method),
                v(&config.name),
                v(&config.note),
                v(&config.params),
                v(config.status),
                v(&config.datasource_id),
                v(config.previlege),
                v(&config.group_id),
                v(&config.cache_plugin),
                v(&config.cache_plugin_params),
                v(&config.create_time),
                v(&config.update_time),
                v(&config.content_type),
                v(config.open_trans),
                v(&config.json_param),
            ],
        )
        .await?;
    }
    for sql in &bundle.sql {
        execute_tx(
            db,
            &txn,
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
    txn.commit().await?;
    Ok(())
}

async fn execute_tx(
    db: &DbConn,
    txn: &DatabaseTransaction,
    sql: &str,
    args: Vec<Value>,
) -> anyhow::Result<u64> {
    let result = txn.execute(db.statement(sql, args)).await?;
    Ok(result.rows_affected())
}

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
        if count_first(
            db,
            "select count(1) as count from api_group where id = ?",
            vec![v(id)],
        )
        .await
            > 0
        {
            anyhow::bail!("group id already exists: {}", id);
        }
        if count_first(
            db,
            "select count(1) as count from api_group where name = ?",
            vec![v(name)],
        )
        .await
            > 0
        {
            anyhow::bail!("group name already exists: {}", name);
        }
    }
    Ok(())
}

async fn validate_import_api_configs(db: &DbConn, bundle: &ApiConfigExport) -> anyhow::Result<()> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();
    let imported_ids = bundle
        .api
        .iter()
        .filter_map(|config| config.id.as_deref().map(str::trim))
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();

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
        if count_first(
            db,
            "select count(1) as count from api_config where id = ?",
            vec![v(id)],
        )
        .await
            > 0
        {
            anyhow::bail!("api id already exists: {}", id);
        }
        if count_first(
            db,
            "select count(1) as count from api_config where path = ?",
            vec![v(path)],
        )
        .await
            > 0
        {
            anyhow::bail!("api path already exists: {}", path);
        }
        if let Some(group_id) = config
            .group_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if count_first(
                db,
                "select count(1) as count from api_group where id = ?",
                vec![v(group_id)],
            )
            .await
                == 0
            {
                anyhow::bail!("api group does not exist: {}", group_id);
            }
        }
        if let Some(datasource_id) = config
            .datasource_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if count_first(
                db,
                "select count(1) as count from datasource where id = ?",
                vec![v(datasource_id)],
            )
            .await
                == 0
            {
                anyhow::bail!("datasource does not exist: {}", datasource_id);
            }
        }
    }

    for sql in &bundle.sql {
        let api_id = sql.api_id.as_deref().unwrap_or("").trim();
        if api_id.is_empty() {
            anyhow::bail!("sql api_id is required");
        }
        if !imported_ids.contains(api_id) {
            anyhow::bail!("sql references unknown api id: {}", api_id);
        }
    }
    Ok(())
}

pub async fn search_api_configs(
    db: &DbConn,
    keyword: Option<&str>,
    field: Option<&str>,
    group_id: Option<&str>,
) -> anyhow::Result<Vec<ApiConfig>> {
    let group_filter = group_id.unwrap_or("").trim();
    let word = keyword.unwrap_or("").trim();
    if word.is_empty() && group_filter.is_empty() {
        return select_all_api_configs(db).await;
    }

    let mut sql = format!("select {API_COLUMNS} from api_config where 1=1");
    let mut args = Vec::new();
    if !group_filter.is_empty() {
        sql.push_str(" and group_id = ?");
        args.push(v(group_filter));
    }
    if !word.is_empty() {
        let like = format!("%{}%", word);
        match field.unwrap_or("") {
            "name" => {
                sql.push_str(" and name like ?");
                args.push(v(like));
            }
            "path" => {
                sql.push_str(" and path like ?");
                args.push(v(like));
            }
            "note" => {
                sql.push_str(" and note like ?");
                args.push(v(like));
            }
            _ => {
                sql.push_str(" and (name like ? or path like ? or note like ?)");
                args.push(v(&like));
                args.push(v(&like));
                args.push(v(like));
            }
        }
    }
    sql.push_str(" order by update_time desc");
    let mut configs = db::query_as(db, &sql, args).await?;
    fill_api_children_for_configs(db, &mut configs).await?;
    Ok(configs)
}

pub async fn select_api_by_id(db: &DbConn, id: &str) -> anyhow::Result<Option<ApiConfig>> {
    db::query_one_as(
        db,
        &format!("select {API_COLUMNS} from api_config where id = ?"),
        vec![v(id)],
    )
    .await
}

pub async fn select_api_by_path_online(
    db: &DbConn,
    path: &str,
) -> anyhow::Result<Option<ApiConfig>> {
    db::query_one_as(
        db,
        &format!("select {API_COLUMNS} from api_config where path = ? and status = 1"),
        vec![v(path)],
    )
    .await
}

pub async fn load_api_detail(db: &DbConn, id: &str) -> anyhow::Result<Option<ApiConfig>> {
    let Some(mut config) = select_api_by_id(db, id).await? else {
        return Ok(None);
    };
    fill_api_children(db, &mut config).await?;
    Ok(Some(config))
}

pub async fn fill_api_children(db: &DbConn, config: &mut ApiConfig) -> anyhow::Result<()> {
    let Some(id) = config.id.as_deref() else {
        return Ok(());
    };
    config.sql_list = select_api_sqls(db, id).await?;
    let alarms = select_api_alarms(db, id).await?;
    if let Some(alarm) = alarms.into_iter().next() {
        config.alarm_plugin = alarm.alarm_plugin;
        config.alarm_plugin_param = alarm.alarm_plugin_param;
    }
    Ok(())
}

async fn fill_api_children_for_configs(
    db: &DbConn,
    configs: &mut [ApiConfig],
) -> anyhow::Result<()> {
    for config in configs {
        fill_api_children(db, config).await?;
    }
    Ok(())
}

pub async fn select_api_sqls(db: &DbConn, api_id: &str) -> anyhow::Result<Vec<ApiSql>> {
    db::query_as(
        db,
        "select id, api_id, sql_text, transform_plugin, transform_plugin_params from api_sql where api_id = ? order by id asc",
        vec![v(api_id)],
    )
    .await
}

pub async fn select_api_alarms(db: &DbConn, api_id: &str) -> anyhow::Result<Vec<ApiAlarm>> {
    db::query_as(
        db,
        "select api_id, alarm_plugin, alarm_plugin_param from api_alarm where api_id = ?",
        vec![v(api_id)],
    )
    .await
}

pub async fn count_api_path(db: &DbConn, path: &str, exclude_id: Option<&str>) -> i64 {
    if let Some(id) = exclude_id {
        count_first(
            db,
            "select count(1) as count from api_config where path = ? and id <> ?",
            vec![v(path), v(id)],
        )
        .await
    } else {
        count_first(
            db,
            "select count(1) as count from api_config where path = ?",
            vec![v(path)],
        )
        .await
    }
}

pub async fn insert_api_config(db: &DbConn, config: &ApiConfig) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v(&config.id),
            v(&config.path),
            v(&config.method),
            v(&config.name),
            v(&config.note),
            v(&config.params),
            v(config.status),
            v(&config.datasource_id),
            v(config.previlege),
            v(&config.group_id),
            v(&config.cache_plugin),
            v(&config.cache_plugin_params),
            v(&config.create_time),
            v(&config.update_time),
            v(&config.content_type),
            v(config.open_trans),
            v(&config.json_param),
        ],
    )
    .await?;
    replace_api_children(db, config).await?;
    Ok(())
}

pub async fn update_api_config(db: &DbConn, config: &ApiConfig) -> anyhow::Result<()> {
    db::execute(
        db,
        "update api_config set path = ?, method = ?, name = ?, note = ?, params = ?, status = ?, datasource_id = ?, previlege = ?, group_id = ?, cache_plugin = ?, cache_plugin_params = ?, update_time = ?, content_type = ?, open_trans = ?, json_param = ? where id = ?",
        vec![
            v(&config.path),
            v(&config.method),
            v(&config.name),
            v(&config.note),
            v(&config.params),
            v(config.status),
            v(&config.datasource_id),
            v(config.previlege),
            v(&config.group_id),
            v(&config.cache_plugin),
            v(&config.cache_plugin_params),
            v(&config.update_time),
            v(&config.content_type),
            v(config.open_trans),
            v(&config.json_param),
            v(&config.id),
        ],
    )
    .await?;
    replace_api_children(db, config).await?;
    Ok(())
}

async fn replace_api_children(db: &DbConn, config: &ApiConfig) -> anyhow::Result<()> {
    let api_id = config.id.as_deref().unwrap_or("");
    db::execute(db, "delete from api_sql where api_id = ?", vec![v(api_id)]).await?;
    for sql in &config.sql_list {
        db::execute(
            db,
            "insert into api_sql (api_id, sql_text, transform_plugin, transform_plugin_params) values (?, ?, ?, ?)",
            vec![
                v(api_id),
                v(&sql.sql_text),
                v(&sql.transform_plugin),
                v(&sql.transform_plugin_params),
            ],
        )
        .await?;
    }

    db::execute(
        db,
        "delete from api_alarm where api_id = ?",
        vec![v(api_id)],
    )
    .await?;
    if config
        .alarm_plugin
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Ok(());
    }
    db::execute(
        db,
        "insert into api_alarm (api_id, alarm_plugin, alarm_plugin_param) values (?, ?, ?)",
        vec![
            v(api_id),
            v(&config.alarm_plugin),
            v(&config.alarm_plugin_param),
        ],
    )
    .await?;
    Ok(())
}

pub async fn delete_api_config(db: &DbConn, id: &str) -> anyhow::Result<()> {
    db::execute(db, "delete from api_config where id = ?", vec![v(id)]).await?;
    db::execute(db, "delete from api_sql where api_id = ?", vec![v(id)]).await?;
    db::execute(db, "delete from api_alarm where api_id = ?", vec![v(id)]).await?;
    Ok(())
}

pub async fn set_api_status(db: &DbConn, id: &str, status: i32) -> anyhow::Result<()> {
    db::execute(
        db,
        "update api_config set status = ?, update_time = ? where id = ?",
        vec![v(status), v(now_string()), v(id)],
    )
    .await?;
    Ok(())
}

pub async fn select_groups(db: &DbConn) -> anyhow::Result<Vec<ApiGroup>> {
    db::query_as(
        db,
        "select id, name from api_group order by name asc",
        vec![],
    )
    .await
}

pub async fn insert_group(db: &DbConn, group: &ApiGroup) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert into api_group (id, name) values (?, ?)",
        vec![v(&group.id), v(&group.name)],
    )
    .await?;
    Ok(())
}

pub async fn delete_group(db: &DbConn, id: &str) -> anyhow::Result<()> {
    db::execute(db, "delete from api_group where id = ?", vec![v(id)]).await?;
    Ok(())
}

pub async fn select_user(
    db: &DbConn,
    username: &str,
    password: &str,
) -> anyhow::Result<Option<User>> {
    db::query_one_as(
        db,
        "select id, username, password from user where username = ? and password = ? limit 1",
        vec![v(username), v(password)],
    )
    .await
}

pub async fn reset_admin_password(db: &DbConn, password: &str) -> anyhow::Result<()> {
    db::execute(
        db,
        "update user set password = ? where username = 'admin'",
        vec![v(password)],
    )
    .await?;
    Ok(())
}

pub async fn select_apps(db: &DbConn) -> anyhow::Result<Vec<AppInfo>> {
    db::query_as(
        db,
        &format!("select {APP_COLUMNS} from app_info order by id asc"),
        vec![],
    )
    .await
}

pub async fn insert_app(db: &DbConn, app: &AppInfo) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert into app_info (id, name, note, secret, expire_desc, expire_duration, token, expire_at) values (?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v(&app.id),
            v(&app.name),
            v(&app.note),
            v(&app.secret),
            v(&app.expire_desc),
            v(app.expire_duration),
            v(&app.token),
            v(app.expire_at),
        ],
    )
    .await?;
    Ok(())
}

pub async fn delete_app(db: &DbConn, id: &str) -> anyhow::Result<()> {
    db::execute(db, "delete from app_info where id = ?", vec![v(id)]).await?;
    db::execute(db, "delete from api_auth where app_id = ?", vec![v(id)]).await?;
    Ok(())
}

pub async fn select_app_by_secret(
    db: &DbConn,
    app_id: &str,
    secret: &str,
) -> anyhow::Result<Option<AppInfo>> {
    db::query_one_as(
        db,
        &format!("select {APP_COLUMNS} from app_info where id = ? and secret = ?"),
        vec![v(app_id), v(secret)],
    )
    .await
}

pub async fn update_app_token(
    db: &DbConn,
    app_id: &str,
    token: &str,
    expire_at: i64,
) -> anyhow::Result<()> {
    db::execute(
        db,
        "update app_info set token = ?, expire_at = ? where id = ?",
        vec![v(token), v(expire_at), v(app_id)],
    )
    .await?;
    Ok(())
}

pub async fn select_app_by_token(db: &DbConn, token: &str) -> anyhow::Result<Option<AppInfo>> {
    db::query_one_as(
        db,
        &format!("select {APP_COLUMNS} from app_info where token = ?"),
        vec![v(token)],
    )
    .await
}

pub async fn clear_app_token(db: &DbConn, app_id: &str) -> anyhow::Result<()> {
    db::execute(
        db,
        "update app_info set token = null, expire_at = null where id = ?",
        vec![v(app_id)],
    )
    .await?;
    Ok(())
}

pub async fn replace_app_auth(
    db: &DbConn,
    app_id: &str,
    group_ids: &[String],
) -> anyhow::Result<()> {
    db::execute(db, "delete from api_auth where app_id = ?", vec![v(app_id)]).await?;
    for group_id in group_ids {
        db::execute(
            db,
            "insert into api_auth (app_id, group_id) values (?, ?)",
            vec![v(app_id), v(group_id)],
        )
        .await?;
    }
    Ok(())
}

pub async fn select_app_auth_groups(db: &DbConn, app_id: &str) -> anyhow::Result<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct GroupIdRow {
        #[serde(alias = "group_id")]
        group_id: Option<String>,
    }

    let rows: Vec<GroupIdRow> = db::query_as(
        db,
        "select group_id from api_auth where app_id = ?",
        vec![v(app_id)],
    )
    .await?;
    Ok(rows.into_iter().filter_map(|row| row.group_id).collect())
}

pub async fn insert_access_log(db: &DbConn, log: &AccessLog) -> anyhow::Result<()> {
    db::execute(
        db,
        "insert into access_log (id, url, status, duration, timestamp, ip, app_id, api_id, error) values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v(&log.id),
            v(&log.url),
            v(log.status),
            v(log.duration),
            v(log.timestamp),
            v(&log.ip),
            v(&log.app_id),
            v(&log.api_id),
            v(&log.error),
        ],
    )
    .await?;
    Ok(())
}

pub async fn access_count_by_day(
    db: &DbConn,
    start: i64,
    end: i64,
) -> anyhow::Result<Vec<JsonValue>> {
    #[derive(Debug, FromQueryResult, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AccessDayCount {
        date: Option<String>,
        success_num: i64,
        fail_num: i64,
    }

    let rows = db
        .conn
        .query_all(db.statement(
            "select date(timestamp, 'unixepoch', 'localtime') as date, coalesce(sum(case when status = 200 then 1 else 0 end), 0) as success_num, coalesce(sum(case when status != 200 then 1 else 0 end), 0) as fail_num from access_log where timestamp >= ? and timestamp < ? group by date order by date",
            vec![v(start), v(end)],
        ))
        .await?;
    typed_json_rows::<AccessDayCount>(rows)
}

pub async fn access_success_ratio(db: &DbConn, start: i64, end: i64) -> anyhow::Result<JsonValue> {
    #[derive(Debug, FromQueryResult, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AccessRatio {
        success_num: i64,
        fail_num: i64,
    }

    let rows = db
        .conn
        .query_all(db.statement(
            "select coalesce(sum(case when status = 200 then 1 else 0 end), 0) as success_num, coalesce(sum(case when status != 200 then 1 else 0 end), 0) as fail_num from access_log where timestamp >= ? and timestamp < ?",
            vec![v(start), v(end)],
        ))
        .await?;
    let Some(row) = rows.first() else {
        return Ok(serde_json::json!({"successNum":0,"failNum":0}));
    };
    Ok(serde_json::to_value(AccessRatio::from_query_result(
        row, "",
    )?)?)
}

pub async fn access_top(
    db: &DbConn,
    kind: &str,
    start: i64,
    end: i64,
) -> anyhow::Result<Vec<JsonValue>> {
    #[derive(Debug, FromQueryResult, Serialize)]
    struct AccessTopUrl {
        url: Option<String>,
        num: i64,
    }

    #[derive(Debug, FromQueryResult, Serialize)]
    struct AccessTopApp {
        app_id: Option<String>,
        num: i64,
    }

    #[derive(Debug, FromQueryResult, Serialize)]
    struct AccessTopIp {
        ip: Option<String>,
        num: i64,
    }

    #[derive(Debug, FromQueryResult, Serialize)]
    struct AccessTopDuration {
        url: Option<String>,
        duration: i64,
    }

    match kind {
        "api" => {
            let rows = db
                .conn
                .query_all(db.statement(
                    "select url, count(1) as num from access_log where timestamp >= ? and timestamp < ? group by url order by num desc limit 10",
                    vec![v(start), v(end)],
                ))
                .await?;
            typed_json_rows::<AccessTopUrl>(rows)
        }
        "app" => {
            let rows = db
                .conn
                .query_all(db.statement(
                    "select app_id, count(1) as num from access_log where timestamp >= ? and timestamp < ? and app_id is not null and app_id != '' group by app_id order by num desc limit 10",
                    vec![v(start), v(end)],
                ))
                .await?;
            typed_json_rows::<AccessTopApp>(rows)
        }
        "ip" => {
            let rows = db
                .conn
                .query_all(db.statement(
                    "select ip, count(1) as num from access_log where timestamp >= ? and timestamp < ? group by ip order by num desc limit 10",
                    vec![v(start), v(end)],
                ))
                .await?;
            typed_json_rows::<AccessTopIp>(rows)
        }
        "duration" => {
            let rows = db
                .conn
                .query_all(db.statement(
                    "select url, cast(coalesce(round(avg(duration)), 0) as integer) as duration from access_log where timestamp >= ? and timestamp < ? group by url order by duration desc limit 10",
                    vec![v(start), v(end)],
                ))
                .await?;
            typed_json_rows::<AccessTopDuration>(rows)
        }
        _ => Ok(Vec::new()),
    }
}

pub async fn access_search(
    db: &DbConn,
    start: i64,
    end: i64,
    url: Option<&str>,
    app_id: Option<&str>,
    status: Option<i32>,
    ip: Option<&str>,
    error: Option<&str>,
) -> anyhow::Result<Vec<AccessLog>> {
    let mut sql = "select id, url, status, duration, timestamp, ip, app_id, api_id, error from access_log where timestamp between ? and ?".to_string();
    let mut args = vec![v(start), v(end)];
    if let Some(status) = status {
        sql.push_str(" and status = ?");
        args.push(v(status));
    }
    if let Some(ip) = ip.filter(|value| !value.is_empty()) {
        sql.push_str(" and ip = ?");
        args.push(v(ip));
    }
    if let Some(url) = url.filter(|value| !value.is_empty()) {
        sql.push_str(" and url = ?");
        args.push(v(url));
    }
    if let Some(app_id) = app_id.filter(|value| !value.is_empty()) {
        sql.push_str(" and app_id = ?");
        args.push(v(app_id));
    }
    if let Some(error) = error.filter(|value| !value.is_empty()) {
        sql.push_str(" and error like ?");
        args.push(v(format!("%{error}%")));
    }
    sql.push_str(" order by timestamp desc");
    db::query_as(db, &sql, args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_repository_creates_empty_sqlite_metadata_store() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("metadata").join("data.db");
        let db = init_repository(&format!("sqlite://{}", db_path.display()))
            .await
            .unwrap();

        assert!(db_path.exists());
        assert!(select_user(&db, "admin", "admin").await.unwrap().is_some());
        assert!(select_all_datasources(&db).await.unwrap().is_empty());
        assert!(select_all_api_configs(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn select_all_api_configs_includes_sql_list() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        create_api_config_test_tables(&db).await;
        db::execute(
            &db,
            "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("api-1"),
                v("/demo/view"),
                v("POST"),
                v("View SQL Demo"),
                v(""),
                v("[]"),
                v(1),
                v("ds-1"),
                v(1),
                v("group-1"),
                v(Option::<String>::None),
                v(Option::<String>::None),
                v("2026-05-06 00:00:00"),
                v("2026-05-06 00:00:00"),
                v("application/x-www-form-urlencoded"),
                v(0),
                v(Option::<String>::None),
            ],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into api_sql (api_id, sql_text, transform_plugin, transform_plugin_params) values (?, ?, ?, ?)",
            vec![
                v("api-1"),
                v("select [[ columns | ident_list ]] from demo_items"),
                v("viewSql"),
                v("resultType=list"),
            ],
        )
        .await
        .unwrap();

        let configs = select_all_api_configs(&db).await.unwrap();

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].sql_list.len(), 1);
        assert_eq!(
            configs[0].sql_list[0].transform_plugin.as_deref(),
            Some("viewSql")
        );
    }

    #[tokio::test]
    async fn api_config_method_round_trips() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        create_api_config_test_tables(&db).await;
        db::execute(
            &db,
            "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("api-get"),
                v("demo/items/list"),
                v("GET"),
                v("Demo List"),
                v(""),
                v("[]"),
                v(1),
                v("ds-1"),
                v(1),
                v("group-1"),
                v(None::<String>),
                v(None::<String>),
                v("2026-05-07 10:00:00"),
                v("2026-05-07 10:00:00"),
                v("application/json"),
                v(0),
                v(None::<String>),
            ],
        )
        .await
        .unwrap();

        let config = select_api_by_id(&db, "api-get").await.unwrap().unwrap();

        assert_eq!(config.method.as_deref(), Some("GET"));
    }

    #[tokio::test]
    async fn select_app_auth_groups_decodes_group_id_rows() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        db::execute(
            &db,
            "insert into api_auth (app_id, group_id) values (?, ?)",
            vec![v("app-1"), v("group-1")],
        )
        .await
        .unwrap();

        let groups = select_app_auth_groups(&db, "app-1").await.unwrap();

        assert_eq!(groups, vec!["group-1".to_string()]);
    }

    #[tokio::test]
    async fn export_api_configs_returns_old_compatible_bundle() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        create_api_config_test_tables(&db).await;
        db::execute(
            &db,
            "insert into api_group (id, name) values (?, ?)",
            vec![v("group-1"), v("demo")],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into datasource (id, name, type, url, username, password, driver) values (?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("ds-1"),
                v("SQLite"),
                v("sqlite"),
                v("sqlite::memory:"),
                v(""),
                v(""),
                v("org.sqlite.JDBC"),
            ],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("api-1"),
                v("/demo/items/get"),
                v("POST"),
                v("get"),
                v(""),
                v("[]"),
                v(1),
                v("ds-1"),
                v(0),
                v("group-1"),
                v(Option::<String>::None),
                v(Option::<String>::None),
                v("2026-05-06 00:00:00"),
                v("2026-05-06 00:00:00"),
                v("application/x-www-form-urlencoded"),
                v(0),
                v(Option::<String>::None),
            ],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into api_sql (api_id, sql_text, transform_plugin, transform_plugin_params) values (?, ?, ?, ?)",
            vec![v("api-1"), v("select 1"), v("sql"), v("")],
        )
        .await
        .unwrap();

        let bundle = export_api_configs(&db, &["api-1".to_string()])
            .await
            .unwrap();

        assert_eq!(bundle.api.len(), 1);
        assert_eq!(bundle.sql.len(), 1);
        assert!(bundle.api[0].sql_list.is_empty());
        assert_eq!(bundle.sql[0].api_id.as_deref(), Some("api-1"));
    }

    #[tokio::test]
    async fn import_api_configs_rejects_duplicate_path() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        create_api_config_test_tables(&db).await;
        db::execute(
            &db,
            "insert into api_group (id, name) values (?, ?)",
            vec![v("group-1"), v("demo")],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into datasource (id, name, type, url, username, password, driver) values (?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("ds-1"),
                v("SQLite"),
                v("sqlite"),
                v("sqlite::memory:"),
                v(""),
                v(""),
                v("org.sqlite.JDBC"),
            ],
        )
        .await
        .unwrap();
        db::execute(
            &db,
            "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                v("existing"),
                v("/demo/items/get"),
                v("POST"),
                v("existing"),
                v(""),
                v("[]"),
                v(1),
                v("ds-1"),
                v(0),
                v("group-1"),
                v(Option::<String>::None),
                v(Option::<String>::None),
                v("2026-05-06 00:00:00"),
                v("2026-05-06 00:00:00"),
                v("application/x-www-form-urlencoded"),
                v(0),
                v(Option::<String>::None),
            ],
        )
        .await
        .unwrap();
        let bundle = ApiConfigExport {
            api: vec![ApiConfig {
                id: Some("api-2".to_string()),
                name: Some("new".to_string()),
                note: Some(String::new()),
                path: Some("/demo/items/get".to_string()),
                method: Some("POST".to_string()),
                datasource_id: Some("ds-1".to_string()),
                sql_list: Vec::new(),
                params: Some("[]".to_string()),
                status: Some(1),
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
            sql: vec![ApiSql {
                id: None,
                api_id: Some("api-2".to_string()),
                sql_text: Some("select 1".to_string()),
                transform_plugin: Some("sql".to_string()),
                transform_plugin_params: Some(String::new()),
            }],
        };

        let error = import_api_configs(&db, &bundle).await.unwrap_err();
        assert!(error.to_string().contains("api path already exists"));
    }

    #[tokio::test]
    async fn search_api_configs_filters_note_field_only() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        create_api_config_test_tables(&db).await;
        for (id, path, name, note) in [
            ("api-note", "/items/by-note", "plain", "needle"),
            ("api-name", "/items/other", "needle", "plain"),
        ] {
            db::execute(
                &db,
                "insert into api_config (id, path, method, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                vec![
                    v(id),
                    v(path),
                    v("POST"),
                    v(name),
                    v(note),
                    v("[]"),
                    v(1),
                    v(Option::<String>::None),
                    v(0),
                    v(Option::<String>::None),
                    v(Option::<String>::None),
                    v(Option::<String>::None),
                    v("2026-05-06 00:00:00"),
                    v("2026-05-06 00:00:00"),
                    v("application/x-www-form-urlencoded"),
                    v(0),
                    v(Option::<String>::None),
                ],
            )
            .await
            .unwrap();
        }

        let configs = search_api_configs(&db, Some("needle"), Some("note"), None)
            .await
            .unwrap();

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].id.as_deref(), Some("api-note"));
    }

    #[tokio::test]
    async fn access_search_filters_url_status_ip_and_error_like() {
        let db = init_repository("sqlite::memory:").await.unwrap();
        for (id, url, status, ip, error) in [
            (
                "log-1",
                "/api/pg/demo/items/delete",
                405,
                "127.0.0.1",
                Some("Method not allowed"),
            ),
            (
                "log-2",
                "/api/pg/demo/items/delete",
                405,
                "127.0.0.1",
                Some("Body parse failed"),
            ),
            ("log-3", "/api/pg/demo/items/delete", 200, "127.0.0.1", None),
            (
                "log-4",
                "/api/pg/demo/items/qb-list",
                405,
                "10.0.0.2",
                Some("Method not allowed"),
            ),
        ] {
            insert_access_log(
                &db,
                &AccessLog {
                    id: Some(id.to_string()),
                    url: Some(url.to_string()),
                    status: Some(status),
                    duration: Some(12),
                    timestamp: Some(100),
                    ip: Some(ip.to_string()),
                    app_id: None,
                    api_id: None,
                    error: error.map(str::to_string),
                },
            )
            .await
            .unwrap();
        }

        let logs = access_search(
            &db,
            0,
            200,
            Some("/api/pg/demo/items/delete"),
            None,
            Some(405),
            Some("127.0.0.1"),
            Some("not allowed"),
        )
        .await
        .unwrap();

        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].id.as_deref(), Some("log-1"));
    }

    async fn create_api_config_test_tables(db: &DbConn) {
        db::execute(
            db,
            "create table if not exists datasource (
                id text primary key,
                name text,
                note text,
                type text,
                url text,
                username text,
                password text,
                driver text,
                table_sql text,
                create_time text,
                update_time text
            )",
            vec![],
        )
        .await
        .unwrap();
        db::execute(
            db,
            "create table if not exists api_group (
                id text primary key,
                name text
            )",
            vec![],
        )
        .await
        .unwrap();
        db::execute(
            db,
            "create table if not exists api_config (
                id text primary key,
                path text,
                method text,
                name text,
                note text,
                params text,
                status integer,
                datasource_id text,
                previlege integer,
                group_id text,
                cache_plugin text,
                cache_plugin_params text,
                create_time text,
                update_time text,
                content_type text,
                open_trans integer,
                json_param text
            )",
            vec![],
        )
        .await
        .unwrap();
        db::execute(
            db,
            "create table if not exists api_sql (
                id integer primary key autoincrement,
                api_id text,
                sql_text text,
                transform_plugin text,
                transform_plugin_params text
            )",
            vec![],
        )
        .await
        .unwrap();
        db::execute(
            db,
            "create table if not exists api_alarm (
                api_id text,
                alarm_plugin text,
                alarm_plugin_param text
            )",
            vec![],
        )
        .await
        .unwrap();
    }
}
