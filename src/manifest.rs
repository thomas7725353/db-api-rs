use crate::model::{ApiConfigExport, ApiGroup};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MANIFEST_VERSION: &str = "dbapi.manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftTableInput {
    pub datasource_id: String,
    pub table: String,
    pub primary_key: Option<String>,
    pub resource_path: String,
    pub group: ManifestGroup,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftSqlInput {
    pub datasource_id: String,
    pub resource_path: String,
    pub api_id: String,
    pub api_name: String,
    pub group: ManifestGroup,
    pub sql: String,
    #[serde(default = "default_sql_engine")]
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbapiManifest {
    pub version: String,
    pub source: ManifestSource,
    pub group_file: String,
    pub api_file: String,
    pub curl_file: String,
    pub verify_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSource {
    pub datasource_id: String,
    pub table: Option<String>,
    pub primary_key: Option<String>,
    pub resource_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedBundle {
    pub manifest: DbapiManifest,
    pub groups: Vec<ApiGroup>,
    pub api_config: ApiConfigExport,
    pub curl_md: String,
    pub verify_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self {
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl ValidationReport {
    pub fn error(&mut self, message: impl Into<String>) {
        self.success = false;
        self.errors.push(message.into());
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
}

fn default_true() -> bool {
    true
}

fn default_sql_engine() -> String {
    "sql".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn draft_table_input_requires_resource_path_in_manifest_shape() {
        let input: DraftTableInput = serde_json::from_value(json!({
            "datasourceId": "postgres_demo",
            "table": "demo_items",
            "primaryKey": "id",
            "resourcePath": "demo/items",
            "group": {"id": "demo_items_group", "name": "Demo Items"}
        }))
        .unwrap();

        assert_eq!(input.resource_path, "demo/items");
        assert_eq!(input.group.id, "demo_items_group");
    }

    #[test]
    fn validation_report_is_success_when_no_errors() {
        let report = ValidationReport::default();
        assert!(report.success);
    }
}
