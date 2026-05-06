use anyhow::{Result, anyhow};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Environment, Error, ErrorKind};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedViewSql {
    pub sql: String,
}

pub fn render_view_sql(template: &str, input: &JsonValue) -> Result<RenderedViewSql> {
    let mut env = Environment::new();
    env.set_syntax(
        SyntaxConfig::builder()
            .variable_delimiters("[[", "]]")
            .block_delimiters("[%", "%]")
            .comment_delimiters("[#", "#]")
            .build()?,
    );
    env.add_filter("ident", ident_filter);
    env.add_filter("ident_list", ident_list_filter);
    env.add_filter("int", int_filter);

    let tmpl = env.template_from_str(template)?;
    let sql = tmpl.render(input)?;
    Ok(RenderedViewSql { sql })
}

fn ident_filter(value: Value) -> std::result::Result<String, Error> {
    let raw = value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "identifier must be a string"))?;
    validate_identifier(raw).map_err(template_error)?;
    Ok(raw.trim().to_string())
}

fn ident_list_filter(value: Value) -> std::result::Result<String, Error> {
    let mut parts = Vec::new();
    match value.kind() {
        ValueKind::String => {
            for item in value
                .as_str()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                validate_identifier(item).map_err(template_error)?;
                parts.push(item.to_string());
            }
        }
        ValueKind::Seq | ValueKind::Iterable => {
            for item in value.try_iter()? {
                let raw = item.as_str().ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidOperation,
                        "identifier list entries must be strings",
                    )
                })?;
                validate_identifier(raw).map_err(template_error)?;
                parts.push(raw.trim().to_string());
            }
        }
        _ => {}
    }
    if parts.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "identifier list cannot be empty",
        ));
    }
    Ok(parts.join(", "))
}

fn int_filter(value: Value, kwargs: Kwargs) -> std::result::Result<String, Error> {
    let default: Option<i64> = kwargs.get("default")?;
    let max: Option<i64> = kwargs.get("max")?;
    let min: Option<i64> = kwargs.get("min")?;
    kwargs.assert_all_used()?;
    let mut parsed = parse_int_value(&value).unwrap_or(default.unwrap_or(0));
    if let Some(min) = min {
        parsed = parsed.max(min);
    }
    if let Some(max) = max {
        parsed = parsed.min(max);
    }
    Ok(parsed.to_string())
}

fn parse_int_value(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
}

fn validate_identifier(raw: &str) -> Result<()> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Invalid identifier: {}", raw));
    }
    if trimmed == "*" {
        return Ok(());
    }
    let mut segments = trimmed.split('.').peekable();
    while let Some(segment) = segments.next() {
        if segment == "*" {
            if segments.peek().is_none() {
                return Ok(());
            }
            return Err(anyhow!("Invalid identifier: {}", raw));
        }
        if !is_identifier_segment(segment) {
            return Err(anyhow!("Invalid identifier: {}", raw));
        }
    }
    Ok(())
}

fn is_identifier_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn template_error(err: anyhow::Error) -> Error {
    Error::new(ErrorKind::InvalidOperation, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_identifier_list_order_limit_and_offset() {
        let rendered = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]",
            &json!({
                "columns": ["a.id", "a.name", "a.c7"],
                "order_by": "a.c7",
                "limit": 20
            }),
        )
        .unwrap();

        assert_eq!(
            rendered.sql,
            "select a.id, a.name, a.c7 from demo_items order by a.c7 desc limit 20 offset 0"
        );
    }

    #[test]
    fn rejects_unsafe_identifiers() {
        let err = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items",
            &json!({ "columns": ["id", "name; drop table demo_items"] }),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Invalid identifier"));
    }

    #[test]
    fn caps_integer_template_values() {
        let rendered = render_view_sql(
            "limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]",
            &json!({ "limit": 5000, "offset": "3" }),
        )
        .unwrap();

        assert_eq!(rendered.sql, "limit 1000 offset 3");
    }

    #[test]
    fn allows_star_for_qualified_selects() {
        let rendered = render_view_sql(
            "select [[ columns | ident_list ]] from demo_items a",
            &json!({ "columns": ["a.*"] }),
        )
        .unwrap();

        assert_eq!(rendered.sql, "select a.* from demo_items a");
    }
}
