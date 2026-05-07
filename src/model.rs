use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSource {
    pub id: Option<String>,
    pub name: Option<String>,
    pub note: Option<String>,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(rename = "type")]
    pub db_type: Option<String>,
    pub driver: Option<String>,
    #[serde(rename = "tableSql", alias = "table_sql")]
    pub table_sql: Option<String>,
    #[serde(rename = "createTime", alias = "create_time")]
    pub create_time: Option<String>,
    #[serde(rename = "updateTime", alias = "update_time")]
    pub update_time: Option<String>,
    #[serde(default, alias = "edit_password")]
    pub edit_password: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub id: Option<String>,
    pub name: Option<String>,
    pub note: Option<String>,
    pub path: Option<String>,
    #[serde(rename = "method", alias = "http_method")]
    pub method: Option<String>,
    #[serde(rename = "datasourceId", alias = "datasource_id")]
    pub datasource_id: Option<String>,
    #[serde(rename = "sqlList", default)]
    pub sql_list: Vec<ApiSql>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub params: Option<String>,
    pub status: Option<i32>,
    pub previlege: Option<i32>,
    #[serde(rename = "groupId", alias = "group_id")]
    pub group_id: Option<String>,
    #[serde(rename = "cachePlugin", alias = "cache_plugin")]
    pub cache_plugin: Option<String>,
    #[serde(rename = "cachePluginParams", alias = "cache_plugin_params")]
    pub cache_plugin_params: Option<String>,
    #[serde(rename = "createTime", alias = "create_time")]
    pub create_time: Option<String>,
    #[serde(rename = "updateTime", alias = "update_time")]
    pub update_time: Option<String>,
    #[serde(rename = "contentType", alias = "content_type")]
    pub content_type: Option<String>,
    #[serde(rename = "openTrans", alias = "open_trans")]
    pub open_trans: Option<i32>,
    #[serde(rename = "jsonParam", alias = "json_param")]
    pub json_param: Option<String>,
    #[serde(rename = "alarmPlugin")]
    pub alarm_plugin: Option<String>,
    #[serde(rename = "alarmPluginParam")]
    pub alarm_plugin_param: Option<String>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<JsonValue>::deserialize(deserializer)?;
    Ok(value.map(|value| match value {
        JsonValue::String(raw) => raw,
        JsonValue::Null => String::new(),
        other => other.to_string(),
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiSql {
    pub id: Option<i32>,
    #[serde(rename = "apiId", alias = "api_id")]
    pub api_id: Option<String>,
    #[serde(rename = "sqlText", alias = "sql_text")]
    pub sql_text: Option<String>,
    #[serde(rename = "transformPlugin", alias = "transform_plugin")]
    pub transform_plugin: Option<String>,
    #[serde(rename = "transformPluginParams", alias = "transform_plugin_params")]
    pub transform_plugin_params: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiGroup {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfigExport {
    #[serde(default)]
    pub api: Vec<ApiConfig>,
    #[serde(default)]
    pub sql: Vec<ApiSql>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i32>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: Option<String>,
    pub secret: Option<String>,
    pub name: Option<String>,
    pub note: Option<String>,
    #[serde(rename = "expireDesc", alias = "expire_desc")]
    pub expire_desc: Option<String>,
    #[serde(
        rename = "expireDuration",
        alias = "expire_duration",
        default,
        deserialize_with = "deserialize_optional_i64"
    )]
    pub expire_duration: Option<i64>,
    pub token: Option<String>,
    #[serde(
        rename = "expireAt",
        alias = "expire_at",
        default,
        deserialize_with = "deserialize_optional_i64"
    )]
    pub expire_at: Option<i64>,
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<JsonValue>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        JsonValue::Number(number) => number.as_i64(),
        JsonValue::String(raw) => raw.parse::<i64>().ok(),
        _ => None,
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessLog {
    pub id: Option<String>,
    pub url: Option<String>,
    pub status: Option<i32>,
    pub duration: Option<i64>,
    pub timestamp: Option<i64>,
    pub ip: Option<String>,
    #[serde(rename = "appId", alias = "app_id")]
    pub app_id: Option<String>,
    #[serde(rename = "apiId", alias = "api_id")]
    pub api_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiAlarm {
    #[serde(rename = "apiId", alias = "api_id")]
    pub api_id: Option<String>,
    #[serde(rename = "alarmPlugin", alias = "alarm_plugin")]
    pub alarm_plugin: Option<String>,
    #[serde(rename = "alarmPluginParam", alias = "alarm_plugin_param")]
    pub alarm_plugin_param: Option<String>,
}
