use crate::model::DataSource;
use anyhow::Result;
use dashmap::DashMap;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use rbdc_pg::driver::PgDriver;
use rbdc_sqlite::driver::SqliteDriver;

pub struct PoolManager {
    pub pools: DashMap<i32, RBatis>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }

    pub async fn get_or_create(&self, ds: &DataSource) -> Result<RBatis> {
        let id = ds
            .id
            .ok_or_else(|| anyhow::anyhow!("DataSource ID is missing"))?;

        if let Some(rb) = self.pools.get(&id) {
            return Ok(rb.clone());
        }

        let rb = self.create_rbatis(ds).await?;
        self.pools.insert(id, rb.clone());
        Ok(rb)
    }

    #[allow(dead_code)]
    pub fn remove(&self, id: i32) {
        self.pools.remove(&id);
    }

    async fn create_rbatis(&self, ds: &DataSource) -> Result<RBatis> {
        let rb = RBatis::new();
        let mut url = ds.url.as_deref().unwrap_or("").to_string();
        let db_type = ds.db_type.as_deref().unwrap_or("").to_lowercase();

        // Handle credentials if they are provided separately and NOT already in the URL
        if let (Some(user), Some(pass)) = (&ds.username, &ds.password)
            && !url.contains('@')
            && (db_type == "mysql" || db_type == "postgres" || db_type == "postgresql")
            && let Some(pos) = url.find("//")
        {
            let (before, after) = url.split_at(pos + 2);
            url = format!("{}{}:{}@{}", before, user, pass, after);
        }

        match db_type.as_str() {
            "mysql" => {
                let url = url.replace("jdbc:mysql://", "mysql://");
                rb.init(MysqlDriver {}, &url)?;
            }
            "postgres" | "postgresql" => {
                let url = url.replace("jdbc:postgresql://", "postgres://");
                rb.init(PgDriver {}, &url)?;
            }
            "sqlite" => {
                let url = url.replace("jdbc:sqlite:", "sqlite://");
                rb.init(SqliteDriver {}, &url)?;
            }
            _ => return Err(anyhow::anyhow!("Unsupported database type: {}", db_type)),
        }
        Ok(rb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataSource;

    #[tokio::test]
    async fn test_get_or_create_sqlite() {
        let manager = PoolManager::new();
        let ds = DataSource {
            id: Some(1),
            name: Some("test_sqlite".to_string()),
            note: None,
            url: Some("jdbc:sqlite::memory:".to_string()),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };

        let _rb = manager
            .get_or_create(&ds)
            .await
            .expect("Failed to get or create RBatis");

        // Verify it's in the map
        assert!(manager.pools.contains_key(&1));

        // Try to get it again, should be the same instance (cloned)
        let rb2 = manager
            .get_or_create(&ds)
            .await
            .expect("Failed to get again");
        // RBatis doesn't implement PartialEq, but we can check if it works
        rb2.exec("SELECT 1", vec![])
            .await
            .expect("Failed to execute query");
    }

    #[tokio::test]
    async fn test_remove_pool() {
        let manager = PoolManager::new();
        let ds = DataSource {
            id: Some(1),
            name: Some("test_sqlite".to_string()),
            note: None,
            url: Some("jdbc:sqlite::memory:".to_string()),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };

        manager.get_or_create(&ds).await.unwrap();
        assert!(manager.pools.contains_key(&1));

        manager.remove(1);
        assert!(!manager.pools.contains_key(&1));
    }
}
