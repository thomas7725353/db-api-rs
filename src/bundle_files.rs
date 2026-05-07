use crate::cli::{BundleArgs, BundleCommand};
use crate::manifest::GeneratedBundle;
use anyhow::Context;
use std::path::Path;

pub fn write_bundle(dir: &Path, bundle: &GeneratedBundle) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating bundle directory {}", dir.display()))?;
    write_json(&dir.join("dbapi_manifest.json"), &bundle.manifest)?;
    write_json(&dir.join("api_group_config.json"), &bundle.groups)?;
    write_json(&dir.join("api_config.json"), &bundle.api_config)?;
    std::fs::write(dir.join("curl.md"), &bundle.curl_md)
        .with_context(|| format!("writing {}", dir.join("curl.md").display()))?;
    std::fs::write(dir.join("VERIFY.md"), &bundle.verify_md)
        .with_context(|| format!("writing {}", dir.join("VERIFY.md").display()))?;
    Ok(())
}

pub fn read_group_file(dir: &Path) -> anyhow::Result<Vec<crate::model::ApiGroup>> {
    read_json(&dir.join("api_group_config.json"))
}

pub fn read_api_file(dir: &Path) -> anyhow::Result<crate::model::ApiConfigExport> {
    read_json(&dir.join("api_config.json"))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let body = serde_json::to_vec_pretty(value)
        .with_context(|| format!("encoding JSON for {}", path.display()))?;
    std::fs::write(path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let body = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&body).with_context(|| format!("decoding JSON from {}", path.display()))
}

pub async fn run_bundle_command(args: BundleArgs) -> anyhow::Result<()> {
    match args.command {
        BundleCommand::DraftTable(_) => anyhow::bail!("draft-table is not implemented yet"),
        BundleCommand::DraftSql(_) => anyhow::bail!("draft-sql is not implemented yet"),
        BundleCommand::Validate(_) => anyhow::bail!("validate is not implemented yet"),
        BundleCommand::Apply(_) => anyhow::bail!("apply is not implemented yet"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DbapiManifest, ManifestSource};
    use crate::model::{ApiConfig, ApiConfigExport, ApiGroup, ApiSql};

    #[test]
    fn writes_bundle_files() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = GeneratedBundle {
            manifest: DbapiManifest {
                version: "dbapi.manifest.v1".to_string(),
                source: ManifestSource {
                    datasource_id: "postgres_demo".to_string(),
                    table: None,
                    primary_key: None,
                    resource_path: "demo/items".to_string(),
                },
                group_file: "api_group_config.json".to_string(),
                api_file: "api_config.json".to_string(),
                curl_file: "curl.md".to_string(),
                verify_file: "VERIFY.md".to_string(),
            },
            groups: vec![ApiGroup {
                id: Some("demo_group".to_string()),
                name: Some("Demo Group".to_string()),
            }],
            api_config: ApiConfigExport {
                api: vec![ApiConfig {
                    id: Some("demo_items_list".to_string()),
                    name: Some("List demo items".to_string()),
                    note: None,
                    path: Some("demo/items/list".to_string()),
                    method: Some("GET".to_string()),
                    datasource_id: Some("postgres_demo".to_string()),
                    sql_list: Vec::new(),
                    params: None,
                    status: Some(1),
                    previlege: Some(0),
                    group_id: Some("demo_group".to_string()),
                    cache_plugin: None,
                    cache_plugin_params: None,
                    create_time: None,
                    update_time: None,
                    content_type: Some("application/json".to_string()),
                    open_trans: Some(0),
                    json_param: None,
                    alarm_plugin: None,
                    alarm_plugin_param: None,
                }],
                sql: vec![ApiSql {
                    id: Some(7),
                    api_id: Some("demo_items_list".to_string()),
                    sql_text: Some("select * from demo_items".to_string()),
                    transform_plugin: None,
                    transform_plugin_params: None,
                }],
            },
            curl_md: "curl http://127.0.0.1:8520/api/demo/items/list".to_string(),
            verify_md: "Verify demo items list".to_string(),
        };

        write_bundle(dir.path(), &bundle).unwrap();

        assert!(dir.path().join("dbapi_manifest.json").exists());
        assert!(dir.path().join("api_group_config.json").exists());
        assert!(dir.path().join("api_config.json").exists());
        assert!(dir.path().join("curl.md").exists());
        assert!(dir.path().join("VERIFY.md").exists());

        let groups = read_group_file(dir.path()).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id.as_deref(), Some("demo_group"));
        assert_eq!(groups[0].name.as_deref(), Some("Demo Group"));

        let api_config = read_api_file(dir.path()).unwrap();
        assert_eq!(api_config.api.len(), 1);
        assert_eq!(api_config.api[0].id.as_deref(), Some("demo_items_list"));
        assert_eq!(api_config.api[0].path.as_deref(), Some("demo/items/list"));
        assert_eq!(api_config.api[0].group_id.as_deref(), Some("demo_group"));
        assert!(api_config.api[0].sql_list.is_empty());
        assert_eq!(api_config.sql.len(), 1);
        assert_eq!(api_config.sql[0].id, Some(7));
        assert_eq!(api_config.sql[0].api_id.as_deref(), Some("demo_items_list"));
    }
}
