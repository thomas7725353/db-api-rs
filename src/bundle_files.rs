use crate::cli::{BundleArgs, BundleCommand};
use crate::manifest::GeneratedBundle;
use std::path::Path;

pub fn write_bundle(dir: &Path, bundle: &GeneratedBundle) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_json(&dir.join("dbapi_manifest.json"), &bundle.manifest)?;
    write_json(&dir.join("api_group_config.json"), &bundle.groups)?;
    write_json(&dir.join("api_config.json"), &bundle.api_config)?;
    std::fs::write(dir.join("curl.md"), &bundle.curl_md)?;
    std::fs::write(dir.join("VERIFY.md"), &bundle.verify_md)?;
    Ok(())
}

pub fn read_group_file(dir: &Path) -> anyhow::Result<Vec<crate::model::ApiGroup>> {
    read_json(&dir.join("api_group_config.json"))
}

pub fn read_api_file(dir: &Path) -> anyhow::Result<crate::model::ApiConfigExport> {
    read_json(&dir.join("api_config.json"))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let body = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, body)?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let body = std::fs::read(path)?;
    Ok(serde_json::from_slice(&body)?)
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
    use crate::model::ApiConfigExport;

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
            groups: Vec::new(),
            api_config: ApiConfigExport {
                api: Vec::new(),
                sql: Vec::new(),
            },
            curl_md: String::new(),
            verify_md: String::new(),
        };

        write_bundle(dir.path(), &bundle).unwrap();

        assert!(dir.path().join("dbapi_manifest.json").exists());
        assert!(dir.path().join("api_group_config.json").exists());
        assert!(dir.path().join("api_config.json").exists());
        assert!(dir.path().join("curl.md").exists());
        assert!(dir.path().join("VERIFY.md").exists());
    }
}
