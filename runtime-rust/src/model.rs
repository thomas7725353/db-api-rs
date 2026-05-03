use rbatis::crud;
use serde::{Deserialize, Serialize};

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
    pub async fn select_all(rb: &rbatis::RBatis) -> rbatis::Result<Vec<DataSource>> {
        rb.exec_decode("select * from datasource", vec![]).await
    }

    pub async fn select_by_id(rb: &rbatis::RBatis, id: i32) -> rbatis::Result<Option<DataSource>> {
        let vec: Vec<DataSource> = rb.exec_decode("select * from datasource where id = ?", vec![rbs::value!(id)]).await?;
        Ok(vec.into_iter().next())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub note: Option<String>,
    pub path: Option<String>,
    pub datasource_id: Option<i32>,
    pub sql: Option<String>,
    pub params: Option<String>,
    pub status: Option<i32>,
}

crud!(ApiConfig {});

impl ApiConfig {
    pub async fn select_all(rb: &rbatis::RBatis) -> rbatis::Result<Vec<ApiConfig>> {
        rb.exec_decode("select * from api_config", vec![]).await
    }

    pub async fn select_by_path(rb: &rbatis::RBatis, path: &str) -> rbatis::Result<Option<ApiConfig>> {
        let vec: Vec<ApiConfig> = rb.exec_decode("select * from api_config where path = ?", vec![rbs::value!(path)]).await?;
        Ok(vec.into_iter().next())
    }
}
