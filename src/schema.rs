use crate::db::{self, DbConn};
use sea_orm::{ConnectionTrait, DbBackend};
use sea_query::Value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub table: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
    pub primary_key: bool,
    pub nullable: Option<bool>,
    pub default_value: Option<String>,
    pub generated: bool,
}

pub async fn list_tables(data_db: &DbConn) -> anyhow::Result<Vec<String>> {
    let sql = match data_db.backend {
        DbBackend::Sqlite => {
            "select name from sqlite_master where type = 'table' and name not like 'sqlite_%' order by name"
        }
        DbBackend::MySql => {
            "select table_name as name from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' order by table_name"
        }
        DbBackend::Postgres => {
            "select table_name as name from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by table_name"
        }
    };
    let rows = db::query_json(data_db, sql, vec![]).await?;
    Ok(rows.into_iter().filter_map(extract_name).collect())
}

pub async fn inspect_table(data_db: &DbConn, table: &str) -> anyhow::Result<TableSchema> {
    let columns = match data_db.backend {
        DbBackend::Sqlite => inspect_sqlite_table(data_db, table).await?,
        DbBackend::MySql => inspect_mysql_table(data_db, table).await?,
        DbBackend::Postgres => inspect_postgres_table(data_db, table).await?,
    };
    Ok(TableSchema {
        table: table.to_string(),
        columns,
    })
}

async fn inspect_sqlite_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    validate_table_identifier(table)?;
    let sql = format!(
        "select name, type, \"notnull\" as not_null, dflt_value, pk from pragma_table_info(\"{}\")",
        escape_sqlite_identifier(table)
    );
    let rows = data_db
        .conn
        .query_all(data_db.statement(&sql, vec![]))
        .await?;
    rows.into_iter()
        .map(|row| {
            let name = row.try_get_by_index::<String>(0)?;
            let column_type = row.try_get_by_index::<String>(1)?;
            let not_null = row.try_get_by_index::<i64>(2)?;
            let default_value = row.try_get_by_index::<Option<String>>(3)?;
            let pk = row.try_get_by_index::<i64>(4)?;
            let primary_key = pk > 0;
            let nullable = Some(not_null == 0 && !primary_key);
            let generated = primary_key && column_type.to_ascii_lowercase().contains("int");
            Ok(ColumnInfo {
                name,
                column_type,
                primary_key,
                nullable,
                default_value,
                generated,
            })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()
        .map_err(Into::into)
}

async fn inspect_mysql_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let rows = db::query_json(
        data_db,
        "select column_name as name, data_type as type, column_key, is_nullable, column_default, extra from information_schema.columns where table_schema = database() and table_name = ? order by ordinal_position",
        vec![string_value(table)],
    )
    .await?;
    Ok(rows.into_iter().filter_map(mysql_column).collect())
}

async fn inspect_postgres_table(data_db: &DbConn, table: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let rows = db::query_json(
        data_db,
        "select c.column_name as name, c.data_type as type, case when exists (select 1 from information_schema.key_column_usage k join information_schema.table_constraints tc on k.constraint_schema = tc.constraint_schema and k.constraint_name = tc.constraint_name and k.table_schema = tc.table_schema and k.table_name = tc.table_name where k.table_schema = c.table_schema and k.table_name = c.table_name and k.column_name = c.column_name and tc.constraint_type = 'PRIMARY KEY') then 'PRI' else '' end as column_key, c.is_nullable, c.column_default, c.is_identity from information_schema.columns c where c.table_schema = 'public' and c.table_name = $1 order by c.ordinal_position",
        vec![string_value(table)],
    )
    .await?;
    Ok(rows.into_iter().filter_map(postgres_column).collect())
}

fn mysql_column(row: JsonValue) -> Option<ColumnInfo> {
    let object = row.as_object()?;
    let extra = object
        .get("extra")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    Some(ColumnInfo {
        name: object.get("name")?.as_str()?.to_string(),
        column_type: object.get("type")?.as_str()?.to_string(),
        primary_key: object.get("column_key").and_then(JsonValue::as_str) == Some("PRI"),
        nullable: object
            .get("is_nullable")
            .and_then(JsonValue::as_str)
            .map(|value| value.eq_ignore_ascii_case("YES")),
        default_value: object
            .get("column_default")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        generated: extra.to_ascii_lowercase().contains("auto_increment"),
    })
}

fn postgres_column(row: JsonValue) -> Option<ColumnInfo> {
    let object = row.as_object()?;
    let default_value = object
        .get("column_default")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let is_identity = object
        .get("is_identity")
        .and_then(JsonValue::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("YES"));
    let generated = is_identity
        || default_value
            .as_deref()
            .is_some_and(|value| value.contains("nextval("));
    Some(ColumnInfo {
        name: object.get("name")?.as_str()?.to_string(),
        column_type: object.get("type")?.as_str()?.to_string(),
        primary_key: object.get("column_key").and_then(JsonValue::as_str) == Some("PRI"),
        nullable: object
            .get("is_nullable")
            .and_then(JsonValue::as_str)
            .map(|value| value.eq_ignore_ascii_case("YES")),
        default_value,
        generated,
    })
}

fn extract_name(row: JsonValue) -> Option<String> {
    match row {
        JsonValue::Object(object) => object
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .or_else(|| object.into_values().next()?.as_str().map(str::to_string)),
        JsonValue::String(value) => Some(value),
        _ => None,
    }
}

fn string_value(value: &str) -> Value {
    Value::String(Some(Box::new(value.to_string())))
}

pub fn validate_table_identifier(value: &str) -> anyhow::Result<()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow::anyhow!("table is required"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(anyhow::anyhow!("Invalid table: {}", value));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(anyhow::anyhow!("Invalid table: {}", value));
    }
    Ok(())
}

fn escape_sqlite_identifier(value: &str) -> String {
    value.replace('"', "\"\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_columns_include_primary_key_and_defaults() {
        let db = db::connect_metadata("sqlite::memory:").await.unwrap();
        db::execute(
            &db,
            "create table demo_items (id integer primary key autoincrement, name text not null, status text default 'active', note text)",
            vec![],
        )
        .await
        .unwrap();

        let schema = inspect_table(&db, "demo_items").await.unwrap();
        let column = |name: &str| {
            schema
                .columns
                .iter()
                .find(|column| column.name == name)
                .unwrap()
        };

        assert!(column("id").primary_key);
        assert!(column("id").generated);
        assert_eq!(column("name").nullable, Some(false));
        assert_eq!(column("status").default_value.as_deref(), Some("'active'"));
    }

    #[test]
    fn rejects_unsafe_sqlite_table_identifier() {
        assert!(validate_table_identifier("users").is_ok());
        assert!(validate_table_identifier("user_2026").is_ok());
        assert!(validate_table_identifier("").is_err());
        assert!(validate_table_identifier("users;drop").is_err());
        assert!(validate_table_identifier("public.users").is_err());
        assert!(validate_table_identifier("1users").is_err());
    }

    #[test]
    fn escapes_sqlite_identifier_quotes_defensively() {
        assert_eq!(escape_sqlite_identifier("a\"b"), "a\"\"b");
    }

    #[test]
    fn mysql_column_maps_primary_key_nullable_default_and_auto_increment() {
        let column = mysql_column(serde_json::json!({
            "name": "id",
            "type": "bigint",
            "column_key": "PRI",
            "is_nullable": "NO",
            "column_default": null,
            "extra": "auto_increment"
        }))
        .unwrap();

        assert_eq!(column.name, "id");
        assert_eq!(column.column_type, "bigint");
        assert!(column.primary_key);
        assert_eq!(column.nullable, Some(false));
        assert!(column.generated);
    }
}
