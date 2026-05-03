use rbatis::RBatis;
use rbdc_sqlite::driver::SqliteDriver;

pub async fn init_repository(url: &str) -> anyhow::Result<RBatis> {
    let rb = RBatis::new();
    rb.init(SqliteDriver {}, url)?;
    Ok(rb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DataSource, ApiConfig};

    async fn setup_test_db() -> RBatis {
        let rb = init_repository("sqlite::memory:").await.unwrap();
        // Create tables for in-memory DB
        rb.exec("CREATE TABLE datasource (id INTEGER PRIMARY KEY, name TEXT, note TEXT, type TEXT, url TEXT, username TEXT, password TEXT)", vec![]).await.unwrap();
        rb.exec("CREATE TABLE api_config (id INTEGER PRIMARY KEY, path TEXT, name TEXT, note TEXT, sql TEXT, params TEXT, status INTEGER, datasource_id INTEGER)", vec![]).await.unwrap();
        rb
    }

    #[tokio::test]
    async fn test_load_datasources() {
        let rb = setup_test_db().await;
        
        let ds = DataSource {
            id: Some(1),
            name: Some("test_ds".to_string()),
            note: None,
            url: Some("jdbc:mysql://localhost:3306/db".to_string()),
            username: Some("root".to_string()),
            password: Some("123456".to_string()),
            db_type: Some("mysql".to_string()),
        };
        DataSource::insert(&rb, &ds).await.unwrap();

        let datasources: Vec<DataSource> = DataSource::select_all(&rb).await.unwrap();
        assert!(!datasources.is_empty());
        assert_eq!(datasources[0].name.as_ref().unwrap(), "test_ds");
        assert_eq!(datasources[0].db_type.as_ref().unwrap(), "mysql");
    }

    #[tokio::test]
    async fn test_load_api_configs() {
        let rb = setup_test_db().await;
        
        let config = ApiConfig {
            id: Some(1),
            name: Some("test_api".to_string()),
            note: None,
            path: Some("/test".to_string()),
            datasource_id: Some(1),
            sql: Some("select * from t".to_string()),
            params: None,
            status: Some(1),
        };
        ApiConfig::insert(&rb, &config).await.unwrap();

        let configs: Vec<ApiConfig> = ApiConfig::select_all(&rb).await.unwrap();
        assert!(!configs.is_empty());
        assert_eq!(configs[0].name.as_ref().unwrap(), "test_api");
    }
}
