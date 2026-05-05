use anyhow::{Result, anyhow};
use sea_orm::DbBackend;
use sea_query::{
    Alias, Asterisk, Cond, Expr, Func, MysqlQueryBuilder, Order, PostgresQueryBuilder, Query,
    SelectStatement, SqliteQueryBuilder, Value, Values,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::collections::HashSet;

use crate::db;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryBuilderDsl {
    #[serde(default = "default_type")]
    pub r#type: String,
    pub table: String,
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default = "default_rules")]
    pub rules: RuleGroup,
    #[serde(default)]
    pub order_by: Vec<OrderBy>,
    pub limit: Option<PageSpec>,
    pub offset: Option<PageSpec>,
    #[serde(default)]
    pub count: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct RuleGroup {
    #[serde(default = "default_combinator")]
    pub combinator: String,
    #[serde(default)]
    pub rules: Vec<RuleNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RuleNode {
    Group(RuleGroup),
    Rule(Rule),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub field: String,
    pub operator: String,
    #[serde(default)]
    pub value_source: Option<String>,
    #[serde(default)]
    pub value: JsonValue,
    #[serde(default)]
    pub default_value: Option<JsonValue>,
    #[serde(default)]
    pub skip_when: Vec<SkipWhen>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipWhen {
    Missing,
    Null,
    EmptyString,
    EmptyArray,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBy {
    pub field: String,
    pub direction: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSpec {
    pub param: Option<String>,
    pub default: Option<i64>,
    pub max: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BuiltQuery {
    pub sql: String,
    pub values: Vec<Value>,
    pub count_sql: Option<String>,
    pub count_values: Vec<Value>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedQuery {
    pub sql: String,
    pub values: Vec<JsonValue>,
    pub count_sql: Option<String>,
    pub params: Vec<String>,
}

pub fn build_query(
    dsl: &QueryBuilderDsl,
    input: &JsonValue,
    backend: DbBackend,
) -> Result<BuiltQuery> {
    validate_identifier(&dsl.table, "table")?;
    let select = if dsl.select.is_empty() {
        vec!["*".to_string()]
    } else {
        dsl.select.clone()
    };
    for field in &select {
        if field != "*" {
            validate_identifier(field, "select field")?;
        }
    }

    let mut query = Query::select();
    if select.iter().any(|field| field == "*") {
        query.column(Asterisk);
    } else {
        for field in &select {
            query.column(Alias::new(field));
        }
    }
    query.from(Alias::new(&dsl.table));

    let condition = build_group(&dsl.rules, input)?;
    if !condition.is_empty() {
        query.cond_where(condition.clone());
    }

    for order in &dsl.order_by {
        validate_identifier(&order.field, "order field")?;
        let direction = match order.direction.to_ascii_lowercase().as_str() {
            "asc" => Order::Asc,
            "desc" => Order::Desc,
            other => return Err(anyhow!("Unsupported order direction: {}", other)),
        };
        query.order_by(Alias::new(&order.field), direction);
    }

    let limit = resolve_page(input, dsl.limit.as_ref(), 20, Some(500), "limit")?;
    let offset = resolve_page(input, dsl.offset.as_ref(), 0, None, "offset")?;
    query.limit(limit);
    query.offset(offset);

    let (sql, values) = build_select(query, backend);

    let (count_sql, count_values) = if dsl.count {
        let mut count_query = Query::select();
        count_query
            .expr_as(Func::count(Expr::col(Asterisk)), Alias::new("total"))
            .from(Alias::new(&dsl.table));
        if !condition.is_empty() {
            count_query.cond_where(condition);
        }
        let (sql, values) = build_select(count_query, backend);
        (Some(sql), values)
    } else {
        (None, Values(Vec::new()))
    };

    Ok(BuiltQuery {
        sql,
        values: values.0,
        count_sql,
        count_values: count_values.0,
        limit: Some(limit),
        offset: Some(offset),
    })
}

pub fn parse_preview(
    dsl: &QueryBuilderDsl,
    input: &JsonValue,
    backend: DbBackend,
) -> Result<ParsedQuery> {
    let built = build_query(dsl, input, backend)?;
    Ok(ParsedQuery {
        sql: built.sql,
        values: built.values.iter().map(db_value_to_json).collect(),
        count_sql: built.count_sql,
        params: collect_params(dsl),
    })
}

pub fn collect_params(dsl: &QueryBuilderDsl) -> Vec<String> {
    let mut params = Vec::new();
    collect_group_params(&dsl.rules, &mut params);
    for spec in [&dsl.limit, &dsl.offset].into_iter().flatten() {
        if let Some(param) = &spec.param
            && !param.trim().is_empty()
            && !params.contains(param)
        {
            params.push(param.clone());
        }
    }
    params
}

fn build_select(query: SelectStatement, backend: DbBackend) -> (String, sea_query::Values) {
    match backend {
        DbBackend::MySql => query.build(MysqlQueryBuilder),
        DbBackend::Postgres => query.build(PostgresQueryBuilder),
        DbBackend::Sqlite => query.build(SqliteQueryBuilder),
    }
}

fn build_group(group: &RuleGroup, input: &JsonValue) -> Result<Cond> {
    let mut condition = match group.combinator.to_ascii_lowercase().as_str() {
        "or" => Cond::any(),
        "and" | "" => Cond::all(),
        other => return Err(anyhow!("Unsupported combinator: {}", other)),
    };

    for node in &group.rules {
        match node {
            RuleNode::Group(group) => {
                let nested = build_group(group, input)?;
                if !nested.is_empty() {
                    condition = condition.add(nested);
                }
            }
            RuleNode::Rule(rule) => {
                if let Some(expr) = build_rule(rule, input)? {
                    condition = condition.add(expr);
                }
            }
        }
    }

    Ok(condition)
}

fn build_rule(rule: &Rule, input: &JsonValue) -> Result<Option<sea_query::SimpleExpr>> {
    validate_identifier(&rule.field, "where field")?;
    let op = normalize_operator(&rule.operator);
    let column = Expr::col(Alias::new(&rule.field));

    if op == "null" || op == "is_null" {
        return Ok(Some(column.is_null()));
    }
    if op == "not_null" || op == "is_not_null" {
        return Ok(Some(column.is_null().not()));
    }

    let resolved = resolve_value(rule, input)?;
    if should_skip(&resolved, &rule.skip_when) {
        return Ok(None);
    }
    let value = resolved.value.unwrap_or(JsonValue::Null);

    let expr = match op.as_str() {
        "=" | "==" => column.eq(db::json_to_db_value(value)),
        "!=" | "<>" => column.ne(db::json_to_db_value(value)),
        ">" => column.gt(db::json_to_db_value(value)),
        ">=" => column.gte(db::json_to_db_value(value)),
        "<" => column.lt(db::json_to_db_value(value)),
        "<=" => column.lte(db::json_to_db_value(value)),
        "contains" | "like" => column.like(like_value(value, LikeMode::Contains)?),
        "begins_with" | "beginswith" => column.like(like_value(value, LikeMode::BeginsWith)?),
        "ends_with" | "endswith" => column.like(like_value(value, LikeMode::EndsWith)?),
        "in" => column.is_in(json_array_values(value, "in")?),
        "not_in" | "notin" => column.is_in(json_array_values(value, "notIn")?).not(),
        other => return Err(anyhow!("Unsupported operator: {}", other)),
    };
    Ok(Some(expr))
}

#[derive(Debug)]
struct ResolvedValue {
    missing: bool,
    value: Option<JsonValue>,
}

fn resolve_value(rule: &Rule, input: &JsonValue) -> Result<ResolvedValue> {
    if rule.value_source.as_deref() == Some("param") {
        let param = rule
            .value
            .as_str()
            .ok_or_else(|| anyhow!("param value must be a string for field {}", rule.field))?;
        if let Some(value) = input.get(param) {
            return Ok(ResolvedValue {
                missing: false,
                value: Some(value.clone()),
            });
        }
        if let Some(default) = &rule.default_value {
            return Ok(ResolvedValue {
                missing: false,
                value: Some(default.clone()),
            });
        }
        return Ok(ResolvedValue {
            missing: true,
            value: None,
        });
    }

    Ok(ResolvedValue {
        missing: false,
        value: Some(rule.value.clone()),
    })
}

fn should_skip(value: &ResolvedValue, skip_when: &[SkipWhen]) -> bool {
    skip_when.iter().any(|skip| match skip {
        SkipWhen::Missing => value.missing,
        SkipWhen::Null => matches!(value.value, Some(JsonValue::Null)),
        SkipWhen::EmptyString => {
            matches!(&value.value, Some(JsonValue::String(raw)) if raw.is_empty())
        }
        SkipWhen::EmptyArray => {
            matches!(&value.value, Some(JsonValue::Array(values)) if values.is_empty())
        }
    })
}

fn json_array_values(value: JsonValue, op: &str) -> Result<Vec<Value>> {
    let values = match value {
        JsonValue::Array(values) => values,
        JsonValue::String(raw) => raw
            .split(',')
            .map(|value| JsonValue::String(value.trim().to_string()))
            .filter(|value| value.as_str().is_some_and(|raw| !raw.is_empty()))
            .collect(),
        other => {
            return Err(anyhow!(
                "{} operator requires an array value, got {}",
                op,
                other
            ));
        }
    };
    Ok(values.into_iter().map(db::json_to_db_value).collect())
}

enum LikeMode {
    Contains,
    BeginsWith,
    EndsWith,
}

fn like_value(value: JsonValue, mode: LikeMode) -> Result<String> {
    let raw = match value {
        JsonValue::String(raw) => raw,
        other => other.to_string(),
    };
    Ok(match mode {
        LikeMode::Contains => format!("%{}%", raw),
        LikeMode::BeginsWith => format!("{}%", raw),
        LikeMode::EndsWith => format!("%{}", raw),
    })
}

fn resolve_page(
    input: &JsonValue,
    spec: Option<&PageSpec>,
    fallback: i64,
    hard_max: Option<i64>,
    name: &str,
) -> Result<u64> {
    let Some(spec) = spec else {
        return Ok(fallback.max(0) as u64);
    };
    let mut value = spec
        .param
        .as_deref()
        .and_then(|param| input.get(param))
        .and_then(json_to_i64)
        .or(spec.default)
        .unwrap_or(fallback);
    if value < 0 {
        return Err(anyhow!("{} must be >= 0", name));
    }
    let max = spec.max.or(hard_max);
    if let Some(max) = max
        && value > max
    {
        value = max;
    }
    Ok(value as u64)
}

fn json_to_i64(value: &JsonValue) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

fn normalize_operator(op: &str) -> String {
    op.trim()
        .replace(' ', "_")
        .replace('-', "_")
        .to_ascii_lowercase()
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value == "*" {
        return Ok(());
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!("{} is required", label));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(anyhow!("Invalid {}: {}", label, value));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(anyhow!("Invalid {}: {}", label, value));
    }
    Ok(())
}

fn collect_group_params(group: &RuleGroup, params: &mut Vec<String>) {
    for node in &group.rules {
        match node {
            RuleNode::Group(group) => collect_group_params(group, params),
            RuleNode::Rule(rule) => {
                if rule.value_source.as_deref() == Some("param")
                    && let Some(param) = rule.value.as_str()
                    && !params.iter().any(|item| item == param)
                {
                    params.push(param.to_string());
                }
            }
        }
    }
}

fn db_value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Bool(value) => value.map(JsonValue::Bool).unwrap_or(JsonValue::Null),
        Value::Int(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::BigInt(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::Unsigned(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::BigUnsigned(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::Float(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::Double(value) => value.map(|value| json!(value)).unwrap_or(JsonValue::Null),
        Value::String(value) => value
            .as_ref()
            .map(|value| JsonValue::String((**value).clone()))
            .unwrap_or(JsonValue::Null),
        _ => JsonValue::String(value.to_string()),
    }
}

fn default_type() -> String {
    "queryBuilder".to_string()
}

fn default_rules() -> RuleGroup {
    RuleGroup {
        combinator: default_combinator(),
        rules: Vec::new(),
    }
}

fn default_combinator() -> String {
    "and".to_string()
}

pub fn infer_param_schema(dsl: &QueryBuilderDsl) -> Vec<JsonValue> {
    let mut seen = HashSet::new();
    collect_params(dsl)
        .into_iter()
        .filter(|name| seen.insert(name.clone()))
        .map(|name| json!({ "name": name, "type": "string" }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_query_with_runtime_params_and_count() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "type": "queryBuilder",
            "table": "demo_item",
            "select": ["id", "name", "status"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "status", "operator": "in", "valueSource": "param", "value": "statusList", "skipWhen": ["missing", "empty_array"]},
                    {"field": "name", "operator": "contains", "valueSource": "param", "value": "keyword", "skipWhen": ["missing", "empty_string"]}
                ]
            },
            "orderBy": [{"field": "id", "direction": "desc"}],
            "limit": {"param": "limit", "default": 20, "max": 100},
            "offset": {"param": "offset", "default": 0},
            "count": true
        }))
        .unwrap();

        let built = build_query(
            &dsl,
            &json!({"statusList":["active","pending"],"keyword":"Alpha","limit":10,"offset":5}),
            DbBackend::Sqlite,
        )
        .unwrap();

        assert!(built.sql.contains("FROM \"demo_item\""));
        assert!(built.sql.contains(" IN "));
        assert!(built.sql.contains("\"name\" LIKE"));
        assert_eq!(built.values.len(), 5);
        assert!(built.count_sql.unwrap().contains("COUNT(*)"));
    }

    #[test]
    fn distinguishes_zero_false_and_empty_string() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "price", "operator": ">", "valueSource": "param", "value": "minPrice", "skipWhen": ["missing", "empty_string"]},
                    {"field": "enabled", "operator": "=", "valueSource": "param", "value": "enabled", "skipWhen": ["missing"]}
                ]
            }
        }))
        .unwrap();

        let built = build_query(
            &dsl,
            &json!({"minPrice":0,"enabled":false}),
            DbBackend::Sqlite,
        )
        .unwrap();

        assert_eq!(built.values.len(), 4);
    }

    #[test]
    fn rejects_unsafe_identifier() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item;drop",
            "select": ["id"],
            "rules": {"combinator": "and", "rules": []}
        }))
        .unwrap();

        assert!(build_query(&dsl, &json!({}), DbBackend::Sqlite).is_err());
    }
}
