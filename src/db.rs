use crate::model::DataSource;
use anyhow::{Result, anyhow};
use dashmap::DashMap;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, FromQueryResult, Statement,
};
use sea_query::Value;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct DbConn {
    pub conn: DatabaseConnection,
    pub backend: DbBackend,
}

impl DbConn {
    pub fn statement(&self, sql: &str, values: Vec<Value>) -> Statement {
        Statement::from_sql_and_values(self.backend, sql.to_string(), values)
    }
}

pub struct DbPoolManager {
    pools: DashMap<String, DbConn>,
    sqlite_base_dir: Option<PathBuf>,
}

impl DbPoolManager {
    pub fn new(sqlite_base_dir: Option<PathBuf>) -> Self {
        Self {
            pools: DashMap::new(),
            sqlite_base_dir,
        }
    }

    pub async fn get_or_create(&self, ds: &DataSource) -> Result<DbConn> {
        let id = ds
            .id
            .as_deref()
            .ok_or_else(|| anyhow!("DataSource ID is required"))?;

        if let Some(existing) = self.pools.get(id) {
            return Ok(existing.clone());
        }

        let conn = connect_data_source_with_base(ds, self.sqlite_base_dir.as_deref()).await?;
        self.pools.insert(id.to_string(), conn.clone());
        Ok(conn)
    }

    pub fn remove(&self, id: &str) {
        self.pools.remove(id);
    }
}

pub async fn connect_metadata(url: &str) -> Result<DbConn> {
    connect_url(url).await
}

async fn connect_data_source_with_base(
    ds: &DataSource,
    sqlite_base_dir: Option<&Path>,
) -> Result<DbConn> {
    let db_type = ds.db_type.as_deref().unwrap_or("sqlite");
    let url = ds
        .url
        .as_deref()
        .ok_or_else(|| anyhow!("DataSource URL is required"))?;
    let url = normalize_url_with_base(
        db_type,
        url,
        ds.username.as_deref(),
        ds.password.as_deref(),
        sqlite_base_dir,
    )?;
    connect_url(&url).await
}

async fn connect_url(url: &str) -> Result<DbConn> {
    let conn = Database::connect(url).await?;
    let backend = conn.get_database_backend();
    Ok(DbConn { conn, backend })
}

fn normalize_url_with_base(
    db_type: &str,
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
    sqlite_base_dir: Option<&Path>,
) -> Result<String> {
    let raw = url.trim();
    if raw.is_empty() {
        return Err(anyhow!("DataSource URL is required"));
    }

    let normalized_type = normalize_db_type(db_type);
    match normalized_type.as_str() {
        "sqlite" => normalize_sqlite_url(raw, sqlite_base_dir),
        "mysql" => normalize_server_url("mysql", raw, username, password),
        "postgres" => normalize_server_url("postgres", raw, username, password),
        other => Err(anyhow!("Unsupported database type: {}", other)),
    }
}

fn normalize_db_type(db_type: &str) -> String {
    match db_type.trim().to_lowercase().as_str() {
        "postgresql" | "pg" => "postgres".to_string(),
        "sqlite3" => "sqlite".to_string(),
        other => other.to_string(),
    }
}

fn normalize_sqlite_url(raw: &str, base_dir: Option<&Path>) -> Result<String> {
    if raw == "jdbc:sqlite::memory:" {
        return Ok("sqlite::memory:".to_string());
    }
    if let Some(path) = raw.strip_prefix("sqlite://") {
        return Ok(sqlite_url_from_path_with_query(path, "", base_dir));
    }
    if let Some(path) = raw.strip_prefix("jdbc:sqlite:") {
        if path == ":memory:" {
            return Ok("sqlite::memory:".to_string());
        }
        return Ok(sqlite_url_from_path_with_query(path, "?mode=rwc", base_dir));
    }
    Ok(sqlite_url_from_path_with_query(raw, "?mode=rwc", base_dir))
}

fn sqlite_url_from_path_with_query(
    raw_path: &str,
    default_query: &str,
    base_dir: Option<&Path>,
) -> String {
    let (path, query) = split_sqlite_path_query(raw_path, default_query);
    let resolved = resolve_sqlite_path(path, base_dir);
    format!("sqlite://{}{}", resolved.display(), query)
}

fn split_sqlite_path_query<'a>(raw_path: &'a str, default_query: &'a str) -> (&'a str, &'a str) {
    raw_path
        .split_once('?')
        .map(|(path, _query)| (path, &raw_path[path.len()..]))
        .unwrap_or((raw_path, default_query))
}

fn resolve_sqlite_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    base_dir
        .map(|base| base.join(candidate))
        .unwrap_or_else(|| candidate.to_path_buf())
}

pub fn sqlite_base_dir_from_url(url: &str) -> Option<PathBuf> {
    let raw = url.trim();
    if raw == "sqlite::memory:" || raw == "jdbc:sqlite::memory:" {
        return None;
    }
    let path = raw
        .strip_prefix("sqlite://")
        .or_else(|| raw.strip_prefix("jdbc:sqlite:"))
        .unwrap_or(raw);
    let (path, _) = split_sqlite_path_query(path, "");
    Path::new(path).parent().map(Path::to_path_buf)
}

fn normalize_server_url(
    scheme: &str,
    raw: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String> {
    let jdbc_prefix = match scheme {
        "mysql" => "jdbc:mysql://",
        "postgres" => "jdbc:postgresql://",
        _ => return Err(anyhow!("Unsupported database type: {}", scheme)),
    };
    let native_prefix = format!("{}://", scheme);
    let mut rest = if let Some(rest) = raw.strip_prefix(jdbc_prefix) {
        rest.to_string()
    } else if let Some(rest) = raw.strip_prefix(&native_prefix) {
        rest.to_string()
    } else {
        raw.to_string()
    };

    if rest.contains('@') {
        return Ok(format!("{}://{}", scheme, rest));
    }

    let Some(user) = username.filter(|value| !value.trim().is_empty()) else {
        return Ok(format!("{}://{}", scheme, rest));
    };
    let pass = password.unwrap_or("");
    rest = format!("{}:{}@{}", user, pass, rest);
    Ok(format!("{}://{}", scheme, rest))
}

pub async fn query_json(db: &DbConn, sql: &str, values: Vec<Value>) -> Result<Vec<JsonValue>> {
    let rows = db.conn.query_all(db.statement(sql, values)).await?;
    rows.into_iter()
        .map(|row| JsonValue::from_query_result(&row, "").map_err(Into::into))
        .collect()
}

pub async fn query_one_json(
    db: &DbConn,
    sql: &str,
    values: Vec<Value>,
) -> Result<Option<JsonValue>> {
    Ok(query_json(db, sql, values).await?.into_iter().next())
}

pub async fn query_as<T>(db: &DbConn, sql: &str, values: Vec<Value>) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    query_json(db, sql, values)
        .await?
        .into_iter()
        .map(|row| serde_json::from_value(row).map_err(Into::into))
        .collect()
}

pub async fn query_one_as<T>(db: &DbConn, sql: &str, values: Vec<Value>) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    Ok(query_as(db, sql, values).await?.into_iter().next())
}

pub async fn execute(db: &DbConn, sql: &str, values: Vec<Value>) -> Result<u64> {
    let result = db.conn.execute(db.statement(sql, values)).await?;
    Ok(result.rows_affected())
}

pub fn json_to_db_value(value: JsonValue) -> Value {
    match value {
        JsonValue::Null => Value::String(None),
        JsonValue::Bool(value) => Value::Bool(Some(value)),
        JsonValue::Number(number) if number.is_i64() => Value::BigInt(number.as_i64()),
        JsonValue::Number(number) if number.is_u64() => {
            let value = number.as_u64().and_then(|value| i64::try_from(value).ok());
            Value::BigInt(value)
        }
        JsonValue::Number(number) => Value::Double(number.as_f64()),
        JsonValue::String(value) => Value::String(Some(Box::new(value))),
        other => Value::Json(Some(Box::new(other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_sqlite_urls() {
        assert_eq!(
            normalize_url_with_base("sqlite", "jdbc:sqlite:/tmp/dbapi.db", None, None, None)
                .unwrap(),
            "sqlite:///tmp/dbapi.db?mode=rwc"
        );
        assert_eq!(
            normalize_url_with_base("sqlite", "sqlite://../data.db", None, None, None).unwrap(),
            "sqlite://../data.db"
        );
        assert_eq!(
            normalize_url_with_base("sqlite", "jdbc:sqlite::memory:", None, None, None).unwrap(),
            "sqlite::memory:"
        );
    }

    #[test]
    fn resolves_relative_sqlite_urls_against_metadata_dir() {
        assert_eq!(
            normalize_url_with_base(
                "sqlite",
                "sqlite://data.db",
                None,
                None,
                Some(Path::new("/data"))
            )
            .unwrap(),
            "sqlite:///data/data.db"
        );
        assert_eq!(
            normalize_url_with_base(
                "sqlite",
                "jdbc:sqlite:data.db",
                None,
                None,
                Some(Path::new("/data"))
            )
            .unwrap(),
            "sqlite:///data/data.db?mode=rwc"
        );
    }

    #[test]
    fn extracts_sqlite_metadata_base_dir() {
        assert_eq!(
            sqlite_base_dir_from_url("sqlite:///data/data.db").unwrap(),
            PathBuf::from("/data")
        );
        assert_eq!(
            sqlite_base_dir_from_url("sqlite://../data.db").unwrap(),
            PathBuf::from("..")
        );
    }

    #[test]
    fn normalizes_server_urls() {
        assert_eq!(
            normalize_url_with_base(
                "mysql",
                "jdbc:mysql://127.0.0.1/db",
                Some("u"),
                Some("p"),
                None,
            )
            .unwrap(),
            "mysql://u:p@127.0.0.1/db"
        );
        assert_eq!(
            normalize_url_with_base(
                "postgresql",
                "jdbc:postgresql://127.0.0.1/db",
                Some("u"),
                Some("p"),
                None,
            )
            .unwrap(),
            "postgres://u:p@127.0.0.1/db"
        );
    }
}
