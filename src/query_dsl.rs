use anyhow::{Result, anyhow};
use sea_orm::DbBackend;
use sea_query::{
    Alias, Asterisk, BinOper, Cond, Expr, Func, MysqlQueryBuilder, Order, PostgresQueryBuilder,
    Query, SelectStatement, SimpleExpr, SqliteQueryBuilder, Value, Values,
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
    #[serde(default, alias = "value_source")]
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

    let condition = build_group(&dsl.rules, input, backend)?;
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

fn build_group(group: &RuleGroup, input: &JsonValue, backend: DbBackend) -> Result<Cond> {
    let mut condition = match group.combinator.to_ascii_lowercase().as_str() {
        "or" => Cond::any(),
        "and" | "" => Cond::all(),
        other => return Err(anyhow!("Unsupported combinator: {}", other)),
    };

    for node in &group.rules {
        match node {
            RuleNode::Group(group) => {
                let nested = build_group(group, input, backend)?;
                if !nested.is_empty() {
                    condition = condition.add(nested);
                }
            }
            RuleNode::Rule(rule) => {
                if let Some(expr) = build_rule(rule, input, backend)? {
                    condition = condition.add(expr);
                }
            }
        }
    }

    Ok(condition)
}

fn build_rule(rule: &Rule, input: &JsonValue, backend: DbBackend) -> Result<Option<SimpleExpr>> {
    validate_identifier(&rule.field, "where field")?;
    let op = normalize_operator(&rule.operator);

    if op == "null" || op == "is_null" {
        return Ok(Some(Expr::col(Alias::new(&rule.field)).is_null()));
    }
    if op == "not_null" || op == "notnull" || op == "is_not_null" {
        return Ok(Some(Expr::col(Alias::new(&rule.field)).is_not_null()));
    }

    let resolved = resolve_value(rule, input)?;
    if should_skip(&resolved, &rule.skip_when) {
        return Ok(None);
    }
    let value = resolved.value.unwrap_or(JsonValue::Null);
    let value_is_field = value_source_is(rule, "field");
    let column = || Expr::col(Alias::new(&rule.field));

    let expr = match op.as_str() {
        "=" | "==" => column().eq(value_operand(value, value_is_field)?),
        "!=" | "<>" => column().ne(value_operand(value, value_is_field)?),
        ">" => column().gt(value_operand(value, value_is_field)?),
        ">=" => column().gte(value_operand(value, value_is_field)?),
        "<" => column().lt(value_operand(value, value_is_field)?),
        "<=" => column().lte(value_operand(value, value_is_field)?),
        "contains" | "like" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::Contains,
            false,
            backend,
        )?,
        "begins_with" | "beginswith" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::BeginsWith,
            false,
            backend,
        )?,
        "ends_with" | "endswith" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::EndsWith,
            false,
            backend,
        )?,
        "does_not_contain" | "doesnotcontain" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::Contains,
            true,
            backend,
        )?,
        "does_not_begin_with" | "doesnotbeginwith" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::BeginsWith,
            true,
            backend,
        )?,
        "does_not_end_with" | "doesnotendwith" => like_expr(
            column(),
            value,
            value_is_field,
            LikeMode::EndsWith,
            true,
            backend,
        )?,
        "in" => column().is_in(value_list_operands(value, value_is_field, "in")?),
        "not_in" | "notin" => column()
            .is_in(value_list_operands(value, value_is_field, "notIn")?)
            .not(),
        "between" => {
            let (lower, upper) = between_operands(value, value_is_field, "between")?;
            column().between(lower, upper)
        }
        "not_between" | "notbetween" => {
            let (lower, upper) = between_operands(value, value_is_field, "notBetween")?;
            column().not_between(lower, upper)
        }
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
    if value_source_is(rule, "param") {
        let binding = param_binding(rule)?;
        if let Some(value) = input.get(&binding.param) {
            return Ok(ResolvedValue {
                missing: false,
                value: Some(value.clone()),
            });
        }
        if let Some(default) = binding.default.or_else(|| rule.default_value.clone()) {
            return Ok(ResolvedValue {
                missing: false,
                value: Some(default),
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

fn value_source_is(rule: &Rule, expected: &str) -> bool {
    rule.value_source
        .as_deref()
        .is_some_and(|source| source.eq_ignore_ascii_case(expected))
}

#[derive(Debug)]
struct ParamBinding {
    param: String,
    default: Option<JsonValue>,
}

fn param_binding(rule: &Rule) -> Result<ParamBinding> {
    if let Some(param) = rule.value.as_str() {
        return Ok(ParamBinding {
            param: param.to_string(),
            default: None,
        });
    }

    let Some(object) = rule.value.as_object() else {
        return Err(anyhow!(
            "param value must be a string or object for field {}",
            rule.field
        ));
    };
    let param = object
        .get("param")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            anyhow!(
                "param object must include string param for field {}",
                rule.field
            )
        })?;
    Ok(ParamBinding {
        param: param.to_string(),
        default: object.get("default").cloned(),
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

fn value_operand(value: JsonValue, value_is_field: bool) -> Result<SimpleExpr> {
    if value_is_field {
        return field_operand(value);
    }
    Ok(db::json_to_db_value(value).into())
}

fn value_list_operands(
    value: JsonValue,
    value_is_field: bool,
    op: &str,
) -> Result<Vec<SimpleExpr>> {
    if value_is_field {
        return list_values(value, op)?
            .into_iter()
            .map(field_operand)
            .collect();
    }
    Ok(json_array_values(value, op)?
        .into_iter()
        .map(Into::into)
        .collect())
}

fn between_operands(
    value: JsonValue,
    value_is_field: bool,
    op: &str,
) -> Result<(SimpleExpr, SimpleExpr)> {
    let mut values = list_values(value, op)?;
    if values.len() != 2 {
        return Err(anyhow!("{} operator requires exactly two values", op));
    }
    let upper = values.pop().expect("len checked");
    let lower = values.pop().expect("len checked");
    Ok((
        value_operand(lower, value_is_field)?,
        value_operand(upper, value_is_field)?,
    ))
}

fn field_operand(value: JsonValue) -> Result<SimpleExpr> {
    let Some(field) = value.as_str() else {
        return Err(anyhow!("field value must be a string identifier"));
    };
    validate_identifier(field, "field value")?;
    Ok(Expr::col(Alias::new(field)).into())
}

fn json_array_values(value: JsonValue, op: &str) -> Result<Vec<Value>> {
    Ok(list_values(value, op)?
        .into_iter()
        .map(db::json_to_db_value)
        .collect())
}

fn list_values(value: JsonValue, op: &str) -> Result<Vec<JsonValue>> {
    let values = match value {
        JsonValue::Array(values) => values,
        JsonValue::String(raw) => raw
            .split(',')
            .map(|value| JsonValue::String(value.trim().to_string()))
            .filter(|value| value.as_str().is_some_and(|raw| !raw.is_empty()))
            .collect(),
        other => {
            return Err(anyhow!(
                "{} operator requires an array value or comma-separated string, got {}",
                op,
                other
            ));
        }
    };
    Ok(values)
}

fn like_expr(
    column: Expr,
    value: JsonValue,
    value_is_field: bool,
    mode: LikeMode,
    negated: bool,
    backend: DbBackend,
) -> Result<SimpleExpr> {
    if value_is_field {
        let pattern = field_like_pattern(value, mode, backend)?;
        let op = if negated {
            BinOper::NotLike
        } else {
            BinOper::Like
        };
        return Ok(column.binary(op, pattern));
    }
    let pattern = like_value(value, mode)?;
    Ok(if negated {
        column.not_like(pattern)
    } else {
        column.like(pattern)
    })
}

fn field_like_pattern(value: JsonValue, mode: LikeMode, backend: DbBackend) -> Result<SimpleExpr> {
    let field = field_operand(value)?;
    Ok(match (backend, mode) {
        (DbBackend::MySql, LikeMode::Contains) => {
            Expr::cust_with_expr("CONCAT('%', ?, '%')", field)
        }
        (DbBackend::MySql, LikeMode::BeginsWith) => Expr::cust_with_expr("CONCAT(?, '%')", field),
        (DbBackend::MySql, LikeMode::EndsWith) => Expr::cust_with_expr("CONCAT('%', ?)", field),
        (DbBackend::Postgres, LikeMode::Contains) => {
            Expr::cust_with_expr("('%' || $1 || '%')", field)
        }
        (DbBackend::Postgres, LikeMode::BeginsWith) => Expr::cust_with_expr("($1 || '%')", field),
        (DbBackend::Postgres, LikeMode::EndsWith) => Expr::cust_with_expr("('%' || $1)", field),
        (DbBackend::Sqlite, LikeMode::Contains) => Expr::cust_with_expr("('%' || ? || '%')", field),
        (DbBackend::Sqlite, LikeMode::BeginsWith) => Expr::cust_with_expr("(? || '%')", field),
        (DbBackend::Sqlite, LikeMode::EndsWith) => Expr::cust_with_expr("('%' || ?)", field),
    })
}

#[derive(Clone, Copy)]
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
    op.trim().replace([' ', '-'], "_").to_ascii_lowercase()
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
                if value_source_is(rule, "param")
                    && let Ok(binding) = param_binding(rule)
                    && !params.iter().any(|item| item == &binding.param)
                {
                    params.push(binding.param);
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
    fn mysql_preview_uses_mysql_quoting_and_placeholders() {
        let dsl = QueryBuilderDsl {
            r#type: "queryBuilder".to_string(),
            table: "demo_items".to_string(),
            select: vec!["id".to_string(), "name".to_string()],
            rules: RuleGroup {
                combinator: "and".to_string(),
                rules: vec![RuleNode::Rule(Rule {
                    field: "status".to_string(),
                    operator: "=".to_string(),
                    value_source: Some("param".to_string()),
                    value: json!({"param": "status"}),
                    default_value: None,
                    skip_when: vec![],
                })],
            },
            order_by: vec![OrderBy {
                field: "id".to_string(),
                direction: "desc".to_string(),
            }],
            limit: Some(PageSpec {
                param: None,
                default: Some(20),
                max: Some(100),
            }),
            offset: Some(PageSpec {
                param: None,
                default: Some(0),
                max: None,
            }),
            count: true,
        };

        let preview = parse_preview(&dsl, &json!({"status": "active"}), DbBackend::MySql).unwrap();

        assert!(preview.sql.contains("`demo_items`"));
        assert!(preview.sql.contains("WHERE `status` = ?"));
        assert!(preview.sql.contains("LIMIT ? OFFSET ?"));
        assert!(preview.count_sql.unwrap().contains("COUNT(*) AS `total`"));
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
    fn param_object_uses_inline_default_when_input_missing() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {
                        "field": "status",
                        "operator": "in",
                        "valueSource": "param",
                        "value": {"param": "statusList", "default": ["active", "pending"]},
                        "skipWhen": ["missing", "empty_array"]
                    }
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(built.sql.contains(" IN "));
        assert_eq!(built.values.len(), 4);
        assert_eq!(db_value_to_json(&built.values[0]), json!("active"));
        assert_eq!(db_value_to_json(&built.values[1]), json!("pending"));
    }

    #[test]
    fn param_object_without_default_is_missing_and_can_be_skipped() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {
                        "field": "status",
                        "operator": "=",
                        "valueSource": "param",
                        "value": {"param": "status"},
                        "skipWhen": ["missing"]
                    }
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(!built.sql.contains("WHERE"));
        assert_eq!(built.values.len(), 2);
    }

    #[test]
    fn param_object_collects_param_names() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "status", "operator": "=", "valueSource": "param", "value": {"param": "status"}}
                ]
            }
        }))
        .unwrap();

        assert_eq!(collect_params(&dsl), vec!["status".to_string()]);
    }

    #[test]
    fn supports_native_does_not_like_operators() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "name", "operator": "doesNotContain", "value": "archived"},
                    {"field": "code", "operator": "doesNotBeginWith", "value": "tmp"},
                    {"field": "suffix", "operator": "doesNotEndWith", "value": "old"}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert_eq!(built.sql.matches("NOT LIKE").count(), 3);
        assert_eq!(built.values.len(), 5);
        assert_eq!(db_value_to_json(&built.values[0]), json!("%archived%"));
        assert_eq!(db_value_to_json(&built.values[1]), json!("tmp%"));
        assert_eq!(db_value_to_json(&built.values[2]), json!("%old"));
    }

    #[test]
    fn supports_between_and_not_between_native_operators() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "age", "operator": "between", "value": [18, 65]},
                    {"field": "score", "operator": "notBetween", "value": "10,20"}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(built.sql.contains("\"age\" BETWEEN"));
        assert!(built.sql.contains("\"score\" NOT BETWEEN"));
        assert_eq!(built.values.len(), 6);
        assert_eq!(db_value_to_json(&built.values[0]), json!(18));
        assert_eq!(db_value_to_json(&built.values[1]), json!(65));
        assert_eq!(db_value_to_json(&built.values[2]), json!("10"));
        assert_eq!(db_value_to_json(&built.values[3]), json!("20"));
    }

    #[test]
    fn between_requires_two_values() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "age", "operator": "between", "value": [18]}
                ]
            }
        }))
        .unwrap();

        let err = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap_err();

        assert!(err.to_string().contains("requires exactly two values"));
    }

    #[test]
    fn supports_native_not_null_operator() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "deleted_at", "operator": "notNull"}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(built.sql.contains("\"deleted_at\" IS NOT NULL"));
        assert_eq!(built.values.len(), 2);
    }

    #[test]
    fn supports_field_to_field_comparisons() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "updated_at", "operator": ">=", "valueSource": "field", "value": "created_at"},
                    {"field": "score", "operator": "between", "valueSource": "field", "value": ["min_score", "max_score"]},
                    {"field": "name", "operator": "doesNotContain", "valueSource": "field", "value": "blocked_pattern"}
                ]
            }
        }))
        .unwrap();

        let built = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap();

        assert!(built.sql.contains("\"updated_at\" >= \"created_at\""));
        assert!(
            built
                .sql
                .contains("\"score\" BETWEEN \"min_score\" AND \"max_score\"")
        );
        assert!(built.sql.contains("\"name\" NOT LIKE"));
        assert!(built.sql.contains("\"blocked_pattern\""));
        assert_eq!(built.values.len(), 2);
    }

    #[test]
    fn rejects_invalid_field_value_source_identifier() {
        let dsl: QueryBuilderDsl = serde_json::from_value(json!({
            "table": "demo_item",
            "select": ["id"],
            "rules": {
                "combinator": "and",
                "rules": [
                    {"field": "updated_at", "operator": "=", "valueSource": "field", "value": "created_at;drop"}
                ]
            }
        }))
        .unwrap();

        let err = build_query(&dsl, &json!({}), DbBackend::Sqlite).unwrap_err();

        assert!(err.to_string().contains("Invalid field value"));
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
