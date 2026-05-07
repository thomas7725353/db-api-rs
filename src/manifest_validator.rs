use crate::manifest::ValidationReport;
use crate::model::{ApiConfig, ApiConfigExport, ApiGroup, ApiSql};
use std::collections::{HashMap, HashSet};

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
        let group_id = trimmed(api.group_id.as_deref());

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
        } else if !is_supported_method(method) {
            report.error(format!(
                "{}: {method}",
                api_context("api method is invalid", id, path)
            ));
        }

        if group_id.is_empty() {
            report.error(api_context("api groupId is required", id, path));
        } else if !group_ids.contains(group_id) {
            report.error(format!(
                "{}: {group_id}",
                api_context("api groupId does not exist in bundle", id, path)
            ));
        }

        if !api.sql_list.is_empty() {
            report.error(api_context(
                "api sqlList is not importable; use top-level bundle.sql rows",
                id,
                path,
            ));
        }
    }

    let mut sql_count_by_api_id = HashMap::new();
    for sql_row in &bundle.sql {
        validate_top_level_sql_row(&mut report, sql_row, &api_ids, &mut sql_count_by_api_id);
    }

    for api in &bundle.api {
        let id = trimmed(api.id.as_deref());
        if !id.is_empty() && !sql_count_by_api_id.contains_key(id) {
            let path = trimmed(api.path.as_deref());
            report.error(api_context(
                "api requires at least one top-level sql row",
                id,
                path,
            ));
        }
    }

    report
}

pub async fn validate_against_server(
    client: &crate::dbapi_client::DbapiClient,
    groups: &[ApiGroup],
    bundle: &ApiConfigExport,
) -> anyhow::Result<ValidationReport> {
    let mut report = validate_bundle_shape(groups, bundle);
    if !report.success {
        return Ok(report);
    }

    let datasources = client.list_datasources().await?;
    let existing_groups = client.list_groups().await?;
    let existing_apis = client.list_api_configs().await?;
    validate_server_state(
        &mut report,
        groups,
        bundle,
        &datasources,
        &existing_groups,
        &existing_apis,
    );

    Ok(report)
}

pub(crate) fn validate_server_state(
    report: &mut ValidationReport,
    groups: &[ApiGroup],
    bundle: &ApiConfigExport,
    datasources: &[crate::model::DataSource],
    existing_groups: &[ApiGroup],
    existing_apis: &[ApiConfig],
) {
    let datasource_ids = datasources
        .iter()
        .filter_map(|datasource| datasource.id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    let server_group_ids = existing_groups
        .iter()
        .filter_map(|group| group.id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    let server_group_names = existing_groups
        .iter()
        .filter_map(|group| group.name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<HashSet<_>>();
    let server_api_ids = existing_apis
        .iter()
        .filter_map(|api| api.id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    let server_api_paths = existing_apis
        .iter()
        .filter_map(|api| api.path.as_deref())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect::<HashSet<_>>();

    for group in groups {
        let id = trimmed(group.id.as_deref());
        let name = trimmed(group.name.as_deref());
        if !id.is_empty() && server_group_ids.contains(id) {
            report.error(format!("group id already exists on server: {id}"));
        }
        if !name.is_empty() && server_group_names.contains(name) {
            report.error(format!("group name already exists on server: {name}"));
        }
    }

    for api in &bundle.api {
        let id = trimmed(api.id.as_deref());
        let path = trimmed(api.path.as_deref());
        let datasource_id = trimmed(api.datasource_id.as_deref());
        if !datasource_id.is_empty() && !datasource_ids.contains(datasource_id) {
            report.error(format!(
                "{}: {datasource_id}",
                api_context("datasource does not exist", id, path)
            ));
        }
        if !id.is_empty() && server_api_ids.contains(id) {
            report.error(format!("api id already exists on server: {id}"));
        }
        if !path.is_empty() && server_api_paths.contains(path) {
            report.error(format!("api path already exists on server: {path}"));
        }
    }
}

fn validate_top_level_sql_row(
    report: &mut ValidationReport,
    sql_row: &ApiSql,
    api_ids: &HashSet<String>,
    sql_count_by_api_id: &mut HashMap<String, usize>,
) {
    let api_id = trimmed(sql_row.api_id.as_deref());
    if api_id.is_empty() {
        report.error("sql apiId is required");
    } else if !api_ids.contains(api_id) {
        report.error(format!("sql references unknown api id: {api_id}"));
    } else {
        *sql_count_by_api_id.entry(api_id.to_string()).or_default() += 1;
    }

    if trimmed(sql_row.sql_text.as_deref()).is_empty() {
        report.error(format!("sqlText is required for api id: {api_id}"));
    }
    if trimmed(sql_row.transform_plugin.as_deref()).is_empty() {
        report.error(format!("transformPlugin is required for api id: {api_id}"));
    }
}

fn is_supported_method(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
    )
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
    use crate::model::{ApiConfig, ApiSql, DataSource};

    #[test]
    fn rejects_leading_slash_and_duplicate_paths() {
        let bundle = ApiConfigExport {
            api: vec![api("one", "/demo/items/get"), api("two", "/demo/items/get")],
            sql: vec![sql("one"), sql("two")],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

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
                transform_plugin: Some("sql".to_string()),
                transform_plugin_params: None,
            }],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("sql references unknown api id: missing"))
        );
    }

    #[test]
    fn rejects_api_group_id_missing_from_bundle_groups() {
        let mut api = api("known", "demo/items/get");
        api.group_id = Some("missing_group".to_string());
        let bundle = ApiConfigExport {
            api: vec![api],
            sql: vec![sql("known")],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

        assert!(!report.success);
        assert!(report.errors.iter().any(|error| {
            error.contains("api groupId does not exist in bundle")
                && error.contains("known")
                && error.contains("missing_group")
        }));
    }

    #[test]
    fn rejects_api_with_only_nested_sql_list() {
        let mut api = api("nested", "demo/items/get");
        api.sql_list = vec![sql("nested")];
        let bundle = ApiConfigExport {
            api: vec![api],
            sql: vec![],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

        assert!(!report.success);
        assert!(report.errors.iter().any(|error| {
            error.contains("api sqlList is not importable") && error.contains("nested")
        }));
        assert!(report.errors.iter().any(|error| {
            error.contains("api requires at least one top-level sql row")
                && error.contains("nested")
        }));
    }

    #[test]
    fn rejects_top_level_sql_row_with_empty_sql_text() {
        let bundle = ApiConfigExport {
            api: vec![api("known", "demo/items/get")],
            sql: vec![ApiSql {
                id: Some(1),
                api_id: Some("known".to_string()),
                sql_text: Some("   ".to_string()),
                transform_plugin: Some("sql".to_string()),
                transform_plugin_params: None,
            }],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("sqlText is required for api id: known"))
        );
    }

    #[test]
    fn rejects_invalid_method() {
        let mut api = api("known", "demo/items/get");
        api.method = Some("TRACE".to_string());
        let bundle = ApiConfigExport {
            api: vec![api],
            sql: vec![sql("known")],
        };

        let report = validate_bundle_shape(&groups(), &bundle);

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("api method is invalid") && error.contains("TRACE"))
        );
    }

    #[test]
    fn rejects_server_group_and_api_conflicts() {
        let bundle = ApiConfigExport {
            api: vec![api("known", "demo/items/get")],
            sql: vec![sql("known")],
        };
        let mut report = validate_bundle_shape(&groups(), &bundle);

        validate_server_state(
            &mut report,
            &groups(),
            &bundle,
            &[datasource("ds")],
            &groups(),
            &[api("known", "demo/items/get")],
        );

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("group id already exists on server: group"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("group name already exists on server: Group"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("api id already exists on server: known"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("api path already exists on server: demo/items/get"))
        );
    }

    #[tokio::test]
    async fn local_shape_errors_skip_server_validation() {
        let bundle = ApiConfigExport {
            api: vec![api("known", "/demo/items/get")],
            sql: vec![sql("known")],
        };
        let client = crate::dbapi_client::DbapiClient::new("http://127.0.0.1:9").unwrap();

        let report = validate_against_server(&client, &groups(), &bundle)
            .await
            .unwrap();

        assert!(!report.success);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("api path must not start with /"))
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

    fn groups() -> Vec<ApiGroup> {
        vec![ApiGroup {
            id: Some("group".to_string()),
            name: Some("Group".to_string()),
        }]
    }

    fn sql(api_id: &str) -> ApiSql {
        ApiSql {
            id: Some(1),
            api_id: Some(api_id.to_string()),
            sql_text: Some("select 1".to_string()),
            transform_plugin: Some("sql".to_string()),
            transform_plugin_params: None,
        }
    }

    fn datasource(id: &str) -> DataSource {
        DataSource {
            id: Some(id.to_string()),
            name: Some(id.to_string()),
            note: None,
            url: None,
            username: None,
            password: None,
            db_type: None,
            driver: None,
            table_sql: None,
            create_time: None,
            update_time: None,
            edit_password: false,
        }
    }
}
