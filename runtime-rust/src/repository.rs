use crate::db::{self, DbConn};
use crate::model::{AccessLog, ApiAlarm, ApiConfig, ApiGroup, ApiSql, AppInfo, DataSource, User};
use chrono::Local;
use sea_orm::{ConnectionTrait, FromQueryResult, QueryResult};
use sea_query::Value;
use serde::Serialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub async fn init_repository(url: &str) -> anyhow::Result<DbConn> {
    let db = db::connect_metadata(url).await?;
    ensure_standalone_tables(&db).await?;
    Ok(db)
}

async fn ensure_standalone_tables(db: &DbConn) -> anyhow::Result<()> {
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
    db::query_one_json(db, sql, args)
        .await
        .ok()
        .flatten()
        .and_then(|row| match row {
            JsonValue::Object(map) => map.into_values().next(),
            other => Some(other),
        })
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(0)
}

const DATASOURCE_COLUMNS: &str =
    "id, name, note, type, url, username, password, driver, table_sql, create_time, update_time";
const API_COLUMNS: &str = "id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param";
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
    db::query_as(
        db,
        &format!("select {API_COLUMNS} from api_config order by update_time desc"),
        vec![],
    )
    .await
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
            _ => {
                sql.push_str(" and (name like ? or path like ? or note like ?)");
                args.push(v(&like));
                args.push(v(&like));
                args.push(v(like));
            }
        }
    }
    sql.push_str(" order by update_time desc");
    db::query_as(db, &sql, args).await
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
        "insert into api_config (id, path, name, note, params, status, datasource_id, previlege, group_id, cache_plugin, cache_plugin_params, create_time, update_time, content_type, open_trans, json_param) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            v(&config.id),
            v(&config.path),
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
        "update api_config set path = ?, name = ?, note = ?, params = ?, status = ?, datasource_id = ?, previlege = ?, group_id = ?, cache_plugin = ?, cache_plugin_params = ?, update_time = ?, content_type = ?, open_trans = ?, json_param = ? where id = ?",
        vec![
            v(&config.path),
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
    sql.push_str(" order by timestamp desc");
    db::query_as(db, &sql, args).await
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
