use rbatis::crud;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSource {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub note: Option<String>,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(rename = "type")]
    pub db_type: Option<String>,
}

crud!(DataSource {}, "datasource");

impl DataSource {
    #[allow(dead_code)]
    pub async fn select_all(rb: &rbatis::RBatis) -> rbatis::Result<Vec<DataSource>> {
        rb.exec_decode("select * from datasource", vec![]).await
    }

    pub async fn select_by_id(rb: &rbatis::RBatis, id: i32) -> rbatis::Result<Option<DataSource>> {
        let vec: Vec<DataSource> = rb
            .exec_decode(
                "select * from datasource where id = ?",
                vec![rbs::value!(id)],
            )
            .await?;
        Ok(vec.into_iter().next())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub note: Option<String>,
    pub path: Option<String>,
    #[serde(rename = "datasourceId", alias = "datasource_id")]
    pub datasource_id: Option<i32>,
    pub sql: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub params: Option<String>,
    pub status: Option<i32>,
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

crud!(ApiConfig {});

impl ApiConfig {
    #[allow(dead_code)]
    pub async fn select_all(rb: &rbatis::RBatis) -> rbatis::Result<Vec<ApiConfig>> {
        rb.exec_decode("select * from api_config", vec![]).await
    }

    pub async fn select_by_path_online(
        rb: &rbatis::RBatis,
        path: &str,
    ) -> rbatis::Result<Option<ApiConfig>> {
        match rb
            .exec_decode(
                "select * from api_config where path = ? and status = 1",
                vec![rbs::value!(path)],
            )
            .await
        {
            Ok(config) => Ok(Some(config)),
            Err(err) if err.to_string().contains("decode empty array value") => Ok(None),
            Err(err) if err.to_string().contains("fail type") => Ok(None),
            Err(err) => Err(err),
        }
    }

    #[cfg(test)]
    pub async fn select_by_path(
        rb: &rbatis::RBatis,
        path: &str,
    ) -> rbatis::Result<Option<ApiConfig>> {
        match rb
            .exec_decode(
                "select * from api_config where path = ?",
                vec![rbs::value!(path)],
            )
            .await
        {
            Ok(config) => Ok(Some(config)),
            Err(err) if err.to_string().contains("decode empty array value") => Ok(None),
            Err(err) if err.to_string().contains("fail type") => Ok(None),
            Err(err) => Err(err),
        }
    }
}
