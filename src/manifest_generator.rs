use crate::manifest::{
    DbapiManifest, DraftSqlInput, DraftTableInput, GeneratedBundle, MANIFEST_VERSION,
    ManifestSource,
};
use crate::model::{ApiConfig, ApiConfigExport, ApiGroup, ApiSql};
use crate::schema::{ColumnInfo, TableSchema};
use anyhow::anyhow;
use serde_json::json;

pub fn draft_table_crud_bundle(
    input: DraftTableInput,
    schema: &TableSchema,
) -> anyhow::Result<GeneratedBundle> {
    if input.table != schema.table {
        return Err(anyhow!(
            "input table does not match schema table: {} != {}",
            input.table,
            schema.table
        ));
    }

    validate_identifier("table", &schema.table)?;
    for column in &schema.columns {
        validate_identifier("column", &column.name)?;
    }

    let resource_path = normalize_resource_path(&input.resource_path)?;
    let primary_key = input.primary_key.clone().or_else(|| {
        schema
            .columns
            .iter()
            .find(|column| column.primary_key)
            .map(|column| column.name.clone())
    });
    let Some(primary_key) = primary_key else {
        return Err(anyhow!(
            "primary_key is required when table metadata has no primary key"
        ));
    };
    validate_identifier("primary_key", &primary_key)?;
    if !schema
        .columns
        .iter()
        .any(|column| column.name == primary_key)
    {
        return Err(anyhow!(
            "primary_key does not exist in table: {}",
            primary_key
        ));
    }

    let writable_columns = schema
        .columns
        .iter()
        .filter(|column| column.name != primary_key && !column.generated)
        .cloned()
        .collect::<Vec<_>>();
    if writable_columns.is_empty() {
        return Err(anyhow!("table has no writable columns: {}", schema.table));
    }
    let selected_columns = schema
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let privilege = if input.public { 1 } else { 0 };
    let group_id = input.group.id.clone();
    let datasource_id = input.datasource_id.clone();
    let resource_id = slug_id(&resource_path);

    let mut api = Vec::new();
    let mut sql = Vec::new();

    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{resource_id}_create"),
            path: format!("{resource_path}/create"),
            method: "POST",
            name: format!("Create {}", schema.table),
            note: "Insert a row".to_string(),
            params: params_for_columns(&writable_columns),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: insert_sql(&schema.table, &writable_columns),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{resource_id}_get"),
            path: format!("{resource_path}/get"),
            method: "GET",
            name: format!("Get {}", schema.table),
            note: "Read one row by primary key".to_string(),
            params: params_for_names(&[primary_key.as_str()], schema),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: format!(
                "select {} from {} where {} = ${}",
                selected_columns.join(", "),
                schema.table,
                primary_key,
                primary_key
            ),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{resource_id}_update"),
            path: format!("{resource_path}/update"),
            method: "PATCH",
            name: format!("Update {}", schema.table),
            note: "Update one row by primary key".to_string(),
            params: [
                params_for_names(&[primary_key.as_str()], schema),
                params_for_columns(&writable_columns),
            ]
            .concat(),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: update_sql(&schema.table, &primary_key, &writable_columns),
        },
    );
    push_sql_api(
        &mut api,
        &mut sql,
        SqlApiSpec {
            id: format!("{resource_id}_delete"),
            path: format!("{resource_path}/delete"),
            method: "DELETE",
            name: format!("Delete {}", schema.table),
            note: "Delete one row by primary key".to_string(),
            params: params_for_names(&[primary_key.as_str()], schema),
            datasource_id: &datasource_id,
            group_id: &group_id,
            privilege,
            sql_text: format!(
                "delete from {} where {} = ${}",
                schema.table, primary_key, primary_key
            ),
        },
    );
    push_query_builder_api(
        &mut api,
        &mut sql,
        &resource_path,
        "qb-list",
        schema,
        &datasource_id,
        &group_id,
        privilege,
    );
    push_query_builder_api(
        &mut api,
        &mut sql,
        &resource_path,
        "table",
        schema,
        &datasource_id,
        &group_id,
        privilege,
    );
    push_view_sql_api(
        &mut api,
        &mut sql,
        &resource_path,
        schema,
        &datasource_id,
        &group_id,
        privilege,
    );

    Ok(GeneratedBundle {
        manifest: DbapiManifest {
            version: MANIFEST_VERSION.to_string(),
            source: ManifestSource {
                datasource_id,
                table: Some(schema.table.clone()),
                primary_key: Some(primary_key),
                resource_path: resource_path.clone(),
            },
            group_file: "api_group_config.json".to_string(),
            api_file: "api_config.json".to_string(),
            curl_file: "curl.md".to_string(),
            verify_file: "VERIFY.md".to_string(),
        },
        groups: vec![ApiGroup {
            id: Some(input.group.id),
            name: Some(input.group.name),
        }],
        api_config: ApiConfigExport { api, sql },
        curl_md: generate_curl_md(&resource_path),
        verify_md: generate_verify_md(),
    })
}

pub fn draft_sql_api_bundle(input: DraftSqlInput) -> anyhow::Result<GeneratedBundle> {
    let resource_path = normalize_resource_path(&input.resource_path)?;
    let method = infer_method_from_sql(&input.sql);
    let params = extract_dollar_params(&input.sql)
        .into_iter()
        .map(|name| json!({"name": name, "type": "string"}))
        .collect::<Vec<_>>();

    let api = base_api(
        &input.api_id,
        &resource_path,
        method,
        &input.api_name,
        "Generated from SQL or agent-authored requirement",
        params,
        &input.datasource_id,
        &input.group.id,
        1,
    );
    let sql = ApiSql {
        id: None,
        api_id: Some(input.api_id),
        sql_text: Some(input.sql),
        transform_plugin: Some(input.engine),
        transform_plugin_params: Some(String::new()),
    };

    Ok(GeneratedBundle {
        manifest: DbapiManifest {
            version: MANIFEST_VERSION.to_string(),
            source: ManifestSource {
                datasource_id: input.datasource_id,
                table: None,
                primary_key: None,
                resource_path: resource_path.clone(),
            },
            group_file: "api_group_config.json".to_string(),
            api_file: "api_config.json".to_string(),
            curl_file: "curl.md".to_string(),
            verify_file: "VERIFY.md".to_string(),
        },
        groups: vec![ApiGroup {
            id: Some(input.group.id),
            name: Some(input.group.name),
        }],
        api_config: ApiConfigExport {
            api: vec![api],
            sql: vec![sql],
        },
        curl_md: generate_single_api_curl_md(&resource_path),
        verify_md: generate_verify_md(),
    })
}

struct SqlApiSpec<'a> {
    id: String,
    path: String,
    method: &'a str,
    name: String,
    note: String,
    params: Vec<serde_json::Value>,
    datasource_id: &'a str,
    group_id: &'a str,
    privilege: i32,
    sql_text: String,
}

fn push_sql_api(api: &mut Vec<ApiConfig>, sql: &mut Vec<ApiSql>, spec: SqlApiSpec<'_>) {
    api.push(base_api(
        &spec.id,
        &spec.path,
        spec.method,
        &spec.name,
        &spec.note,
        spec.params,
        spec.datasource_id,
        spec.group_id,
        spec.privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(spec.id),
        sql_text: Some(spec.sql_text),
        transform_plugin: Some("sql".to_string()),
        transform_plugin_params: Some(String::new()),
    });
}

fn push_query_builder_api(
    api: &mut Vec<ApiConfig>,
    sql: &mut Vec<ApiSql>,
    resource_path: &str,
    suffix: &str,
    schema: &TableSchema,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) {
    let id = format!("{}_{}", slug_id(resource_path), suffix.replace('-', "_"));
    let path = format!("{resource_path}/{suffix}");
    let select = schema
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    api.push(base_api(
        &id,
        &path,
        "GET",
        &format!("{} {}", schema.table, suffix),
        "QueryBuilder page API",
        vec![
            json!({"name":"keyword","type":"string"}),
            json!({"name":"limit","type":"bigint"}),
            json!({"name":"offset","type":"bigint"}),
        ],
        datasource_id,
        group_id,
        privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(id),
        sql_text: Some(
            json!({
                "type": "queryBuilder",
                "table": schema.table,
                "select": select,
                "rules": {"combinator":"and","rules":[]},
                "orderBy": default_order(schema),
                "limit": {"param":"limit","default":20,"max":100},
                "offset": {"param":"offset","default":0},
                "count": true
            })
            .to_string(),
        ),
        transform_plugin: Some("queryBuilder".to_string()),
        transform_plugin_params: Some("resultType=page".to_string()),
    });
}

fn push_view_sql_api(
    api: &mut Vec<ApiConfig>,
    sql: &mut Vec<ApiSql>,
    resource_path: &str,
    schema: &TableSchema,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) {
    let id = format!("{}_view_sql_list", slug_id(resource_path));
    let path = format!("{resource_path}/view-sql-list");
    api.push(base_api(
        &id,
        &path,
        "GET",
        &format!("{} View SQL List", schema.table),
        "View/report/analysis API",
        vec![
            json!({"name":"order_by","type":"string"}),
            json!({"name":"limit","type":"bigint"}),
            json!({"name":"offset","type":"bigint"}),
        ],
        datasource_id,
        group_id,
        privilege,
    ));
    sql.push(ApiSql {
        id: None,
        api_id: Some(id.clone()),
        sql_text: Some(format!(
            "select {} from {} a where 1 = 1 order by [[ order_by | ident ]] desc limit [[ limit | int(default=20,max=100) ]] offset [[ offset | int(default=0) ]]",
            select_list(schema),
            schema.table
        )),
        transform_plugin: Some("viewSql".to_string()),
        transform_plugin_params: Some("resultType=page".to_string()),
    });
    sql.push(ApiSql {
        id: None,
        api_id: Some(id),
        sql_text: Some(format!(
            "select count(*) as total from {} a where 1 = 1",
            schema.table
        )),
        transform_plugin: Some("viewSqlCount".to_string()),
        transform_plugin_params: Some(String::new()),
    });
}

fn base_api(
    id: &str,
    path: &str,
    method: &str,
    name: &str,
    note: &str,
    params: Vec<serde_json::Value>,
    datasource_id: &str,
    group_id: &str,
    privilege: i32,
) -> ApiConfig {
    ApiConfig {
        id: Some(id.to_string()),
        name: Some(name.to_string()),
        note: Some(note.to_string()),
        path: Some(path.to_string()),
        method: Some(method.to_string()),
        datasource_id: Some(datasource_id.to_string()),
        sql_list: Vec::new(),
        params: Some(serde_json::to_string(&params).unwrap()),
        status: Some(1),
        previlege: Some(privilege),
        group_id: Some(group_id.to_string()),
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

fn normalize_resource_path(path: &str) -> anyhow::Result<String> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err(anyhow!("resource_path is required"));
    }
    Ok(trimmed.to_string())
}

fn infer_method_from_sql(sql: &str) -> &'static str {
    match sql
        .trim_start()
        .split(|ch: char| ch.is_whitespace() || ch == '(')
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "select" | "with" | "show" => "GET",
        "insert" => "POST",
        "update" => "PATCH",
        "delete" => "DELETE",
        _ => "POST",
    }
}

fn extract_dollar_params(sql: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut chars = sql.char_indices().peekable();
    let mut in_single_quote = false;

    while let Some((_, ch)) = chars.next() {
        if ch == '\'' {
            if in_single_quote && chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
            } else {
                in_single_quote = !in_single_quote;
            }
            continue;
        }

        if in_single_quote || ch != '$' {
            continue;
        }

        let Some((start, first)) = chars.peek().copied() else {
            continue;
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            continue;
        }

        let mut end = start + first.len_utf8();
        chars.next();
        while let Some((idx, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = idx + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let param = sql[start..end].to_string();
        if seen.insert(param.clone()) {
            params.push(param);
        }
    }

    params
}

fn validate_identifier(kind: &str, value: &str) -> anyhow::Result<()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!("{kind} identifier is required"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(anyhow!("invalid {kind} identifier: {value}"));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(anyhow!("invalid {kind} identifier: {value}"));
    }
    Ok(())
}

fn slug_id(resource_path: &str) -> String {
    resource_path.replace(['/', '-'], "_")
}

fn params_for_columns(columns: &[ColumnInfo]) -> Vec<serde_json::Value> {
    columns
        .iter()
        .map(|column| json!({"name": column.name, "type": param_type(&column.column_type)}))
        .collect()
}

fn params_for_names(names: &[&str], schema: &TableSchema) -> Vec<serde_json::Value> {
    names
        .iter()
        .filter_map(|name| schema.columns.iter().find(|column| column.name == *name))
        .map(|column| json!({"name": column.name, "type": param_type(&column.column_type)}))
        .collect()
}

fn param_type(raw: &str) -> &'static str {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("int") {
        "bigint"
    } else if lower.contains("double")
        || lower.contains("real")
        || lower.contains("float")
        || lower.contains("numeric")
        || lower.contains("decimal")
    {
        "double"
    } else if lower.contains("date") || lower.contains("time") {
        "date"
    } else {
        "string"
    }
}

fn insert_sql(table: &str, columns: &[ColumnInfo]) -> String {
    let names = columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    let params = names
        .iter()
        .map(|name| format!("${name}"))
        .collect::<Vec<_>>();
    format!(
        "insert into {table} ({}) values ({})",
        names.join(", "),
        params.join(", ")
    )
}

fn update_sql(table: &str, primary_key: &str, columns: &[ColumnInfo]) -> String {
    let assignments = columns
        .iter()
        .map(|column| format!("{} = ${}", column.name, column.name))
        .collect::<Vec<_>>();
    format!(
        "update {table} set {} where {primary_key} = ${primary_key}",
        assignments.join(", ")
    )
}

fn default_order(schema: &TableSchema) -> Vec<serde_json::Value> {
    schema
        .columns
        .iter()
        .find(|column| column.primary_key)
        .map(|column| vec![json!({"field": column.name, "direction": "desc"})])
        .unwrap_or_default()
}

fn select_list(schema: &TableSchema) -> String {
    schema
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn generate_curl_md(resource_path: &str) -> String {
    format!(
        "# cURL Examples\n\n```bash\ncurl -sS 'http://127.0.0.1:8520/api/{resource_path}/qb-list?limit=20&offset=0'\n```\n"
    )
}

fn generate_single_api_curl_md(resource_path: &str) -> String {
    format!(
        "# cURL Examples\n\n```bash\ncurl -sS 'http://127.0.0.1:8520/api/{resource_path}'\n```\n"
    )
}

fn generate_verify_md() -> String {
    "# Verify\n\n1. Validate generated API bundle.\n2. Apply group config.\n3. Apply API config.\n4. Generate token if APIs are private.\n5. Run cURL examples.\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DraftSqlInput, DraftTableInput, ManifestGroup};
    use crate::schema::{ColumnInfo, TableSchema};

    #[test]
    fn table_bundle_generates_sql_querybuilder_and_view_sql_apis() {
        let schema = TableSchema {
            table: "demo_items".to_string(),
            columns: vec![
                col("id", "integer", true, true),
                col("name", "text", false, false),
                col("status", "text", false, false),
                col("note", "text", false, false),
                col("created_at", "timestamp", false, true),
                col("updated_at", "timestamp", false, true),
            ],
        };
        let bundle = draft_table_crud_bundle(
            DraftTableInput {
                datasource_id: "postgres_demo".to_string(),
                table: "demo_items".to_string(),
                primary_key: Some("id".to_string()),
                resource_path: "demo/items".to_string(),
                group: ManifestGroup {
                    id: "demo_items_group".to_string(),
                    name: "Demo Items".to_string(),
                },
                public: true,
            },
            &schema,
        )
        .unwrap();

        let paths = bundle
            .api_config
            .api
            .iter()
            .map(|api| api.path.as_deref().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "demo/items/create",
                "demo/items/get",
                "demo/items/update",
                "demo/items/delete",
                "demo/items/qb-list",
                "demo/items/table",
                "demo/items/view-sql-list",
            ]
        );

        assert_eq!(bundle.api_config.api.len(), 7);
        assert!(
            bundle
                .api_config
                .sql
                .iter()
                .any(|row| row.transform_plugin.as_deref() == Some("queryBuilder"))
        );
        assert!(
            bundle
                .api_config
                .sql
                .iter()
                .any(|row| row.transform_plugin.as_deref() == Some("viewSql"))
        );
        assert!(
            bundle
                .api_config
                .sql
                .iter()
                .any(|row| row.transform_plugin.as_deref() == Some("viewSqlCount"))
        );
        let view_sql = bundle
            .api_config
            .sql
            .iter()
            .find(|row| row.transform_plugin.as_deref() == Some("viewSql"))
            .and_then(|row| row.sql_text.as_deref())
            .unwrap();
        assert!(view_sql.contains("select id, name"));
        assert!(!view_sql.contains("columns | ident_list"));
        assert!(bundle.curl_md.contains("/api/demo/items/qb-list"));
        assert!(bundle.verify_md.contains("Validate generated API bundle"));
    }

    #[test]
    fn sql_bundle_generates_single_api_with_method_and_params() {
        let bundle = draft_sql_api_bundle(DraftSqlInput {
            datasource_id: "postgres_demo".to_string(),
            resource_path: "demo/items/custom-search".to_string(),
            api_id: "demo_items_custom_search".to_string(),
            api_name: "Demo Items Custom Search".to_string(),
            group: ManifestGroup {
                id: "demo_items_group".to_string(),
                name: "Demo Items".to_string(),
            },
            sql: "select id, name from demo_items where status = $status".to_string(),
            engine: "sql".to_string(),
        })
        .unwrap();

        assert_eq!(bundle.api_config.api[0].method.as_deref(), Some("GET"));
        assert_eq!(
            bundle.api_config.api[0].path.as_deref(),
            Some("demo/items/custom-search")
        );
        assert_eq!(
            bundle.api_config.sql[0].transform_plugin.as_deref(),
            Some("sql")
        );
        assert!(
            bundle.api_config.api[0]
                .params
                .as_deref()
                .unwrap()
                .contains("status")
        );
    }

    #[test]
    fn table_bundle_rejects_tables_with_no_writable_columns() {
        let schema = TableSchema {
            table: "generated_only".to_string(),
            columns: vec![col("id", "integer", true, true)],
        };
        let error = draft_table_crud_bundle(table_input("generated_only"), &schema)
            .unwrap_err()
            .to_string();

        assert!(error.contains("no writable columns"));
    }

    #[test]
    fn table_bundle_rejects_unsafe_identifiers_before_generating_sql() {
        let schema = TableSchema {
            table: "demo_items".to_string(),
            columns: vec![
                col("id", "integer", true, true),
                col("bad-name", "text", false, false),
            ],
        };
        let error = draft_table_crud_bundle(table_input("demo_items"), &schema)
            .unwrap_err()
            .to_string();

        assert!(error.contains("invalid column identifier"));
    }

    fn table_input(table: &str) -> DraftTableInput {
        DraftTableInput {
            datasource_id: "postgres_demo".to_string(),
            table: table.to_string(),
            primary_key: Some("id".to_string()),
            resource_path: "demo/items".to_string(),
            group: ManifestGroup {
                id: "demo_items_group".to_string(),
                name: "Demo Items".to_string(),
            },
            public: true,
        }
    }

    fn col(name: &str, ty: &str, primary_key: bool, generated: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            column_type: ty.to_string(),
            primary_key,
            nullable: Some(!primary_key),
            default_value: None,
            generated,
        }
    }
}
