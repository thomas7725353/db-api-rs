use crate::manifest::ValidationReport;
use crate::model::{ApiConfigExport, ApiGroup};
use std::collections::HashSet;

pub fn validate_bundle_shape(groups: &[ApiGroup], bundle: &ApiConfigExport) -> ValidationReport {
    let mut report = ValidationReport::default();
    let mut group_ids = HashSet::new();
    let mut api_ids = HashSet::new();
    let mut api_paths = HashSet::new();

    for group in groups {
        let id = trimmed(group.id.as_deref());
        let name = trimmed(group.name.as_deref());

        if id.is_empty() {
            report.error("group id is required");
        } else if !group_ids.insert(id.to_string()) {
            report.error(format!("duplicate group id in bundle: {id}"));
        }

        if name.is_empty() {
            report.error(format!("group name is required: {id}"));
        }
    }

    for api in &bundle.api {
        let id = trimmed(api.id.as_deref());
        let path = trimmed(api.path.as_deref());
        let datasource_id = trimmed(api.datasource_id.as_deref());
        let method = trimmed(api.method.as_deref());

        if id.is_empty() {
            report.error(api_context("api id is required", id, path));
        } else if !api_ids.insert(id.to_string()) {
            report.error(format!("duplicate api id in bundle: {id}"));
        }

        if path.is_empty() {
            report.error(api_context("api path is required", id, path));
        } else {
            if path.starts_with('/') {
                report.error(format!("api path must not start with /: {path}"));
            }
            if !api_paths.insert(path.to_string()) {
                report.error(format!("duplicate api path in bundle: {path}"));
            }
        }

        if datasource_id.is_empty() {
            report.error(api_context("api datasourceId is required", id, path));
        }
        if method.is_empty() {
            report.error(api_context("api method is required", id, path));
        }

        for sql_row in &api.sql_list {
            validate_sql_api_id(&mut report, sql_row.api_id.as_deref(), &api_ids);
        }
    }

    for sql_row in &bundle.sql {
        validate_sql_api_id(&mut report, sql_row.api_id.as_deref(), &api_ids);
    }

    report
}

pub async fn validate_against_server(
    client: &crate::dbapi_client::DbapiClient,
    groups: &[ApiGroup],
    bundle: &ApiConfigExport,
) -> anyhow::Result<ValidationReport> {
    let mut report = validate_bundle_shape(groups, bundle);
    let datasources = client.list_datasources().await?;
    let datasource_ids = datasources
        .iter()
        .filter_map(|datasource| datasource.id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();

    for api in &bundle.api {
        let datasource_id = trimmed(api.datasource_id.as_deref());
        if !datasource_id.is_empty() && !datasource_ids.contains(datasource_id) {
            let id = trimmed(api.id.as_deref());
            let path = trimmed(api.path.as_deref());
            report.error(format!(
                "{}: {datasource_id}",
                api_context("datasource does not exist", id, path)
            ));
        }
    }

    Ok(report)
}

fn validate_sql_api_id(
    report: &mut ValidationReport,
    api_id: Option<&str>,
    api_ids: &HashSet<String>,
) {
    let api_id = trimmed(api_id);
    if api_id.is_empty() {
        report.error("sql apiId is required");
    } else if !api_ids.contains(api_id) {
        report.error(format!("sql references unknown api id: {api_id}"));
    }
}

fn trimmed(value: Option<&str>) -> &str {
    value.unwrap_or("").trim()
}

fn api_context(message: &str, id: &str, path: &str) -> String {
    match (id.is_empty(), path.is_empty()) {
        (true, true) => message.to_string(),
        (false, true) => format!("{message}: api id {id}"),
        (true, false) => format!("{message}: api path {path}"),
        (false, false) => format!("{message}: api id {id}, path {path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiConfig, ApiSql};

    #[test]
    fn rejects_leading_slash_and_duplicate_paths() {
        let bundle = ApiConfigExport {
            api: vec![api("one", "/demo/items/get"), api("two", "/demo/items/get")],
            sql: vec![],
        };

        let report = validate_bundle_shape(&[], &bundle);

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("must not start with /"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("duplicate api path"))
        );
    }

    #[test]
    fn rejects_sql_row_with_unknown_api_id() {
        let bundle = ApiConfigExport {
            api: vec![api("known", "demo/items/get")],
            sql: vec![ApiSql {
                id: Some(1),
                api_id: Some("missing".to_string()),
                sql_text: Some("select 1".to_string()),
                transform_plugin: None,
                transform_plugin_params: None,
            }],
        };

        let report = validate_bundle_shape(&[], &bundle);

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("sql references unknown api id: missing"))
        );
    }

    fn api(id: &str, path: &str) -> ApiConfig {
        ApiConfig {
            id: Some(id.to_string()),
            name: Some(id.to_string()),
            note: None,
            path: Some(path.to_string()),
            method: Some("GET".to_string()),
            datasource_id: Some("ds".to_string()),
            sql_list: Vec::new(),
            params: Some("[]".to_string()),
            status: Some(1),
            previlege: Some(1),
            group_id: Some("group".to_string()),
            cache_plugin: None,
            cache_plugin_params: None,
            create_time: None,
            update_time: None,
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            open_trans: Some(0),
            json_param: None,
            alarm_plugin: None,
            alarm_plugin_param: None,
        }
    }
}
