use anyhow::{Result, anyhow};
use serde_json::Value as JsonValue;
use sqlparser::ast::Statement;
use sqlparser::dialect::{Dialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialectType {
    MySql,
    PostgreSql,
    Sqlite,
}

pub struct SqlTransformer;

impl SqlTransformer {
    pub fn transform(sql: &str, dialect_type: DialectType) -> Result<(String, Vec<String>)> {
        let dialect: Box<dyn Dialect> = match dialect_type {
            DialectType::MySql => Box::new(MySqlDialect {}),
            DialectType::PostgreSql => Box::new(PostgreSqlDialect {}),
            DialectType::Sqlite => Box::new(SQLiteDialect {}),
        };

        let statements = Parser::parse_sql(&*dialect, sql)?;

        if statements.len() != 1 {
            return Err(anyhow!("Only single statement allowed"));
        }

        let statement = &statements[0];

        match statement {
            Statement::Query(_)
            | Statement::Insert { .. }
            | Statement::Update { .. }
            | Statement::Delete { .. } => {}
            _ => {
                return Err(anyhow!(
                    "Only SELECT/INSERT/UPDATE/DELETE statements are allowed"
                ));
            }
        }

        Self::replace_named_params(sql, dialect_type)
    }

    pub fn is_query(sql: &str, dialect_type: DialectType) -> Result<bool> {
        let dialect: Box<dyn Dialect> = match dialect_type {
            DialectType::MySql => Box::new(MySqlDialect {}),
            DialectType::PostgreSql => Box::new(PostgreSqlDialect {}),
            DialectType::Sqlite => Box::new(SQLiteDialect {}),
        };
        let statements = Parser::parse_sql(&*dialect, sql)?;
        Ok(matches!(statements.as_slice(), [Statement::Query(_)]))
    }

    fn replace_named_params(sql: &str, dialect_type: DialectType) -> Result<(String, Vec<String>)> {
        let mut transformed = String::with_capacity(sql.len());
        let mut params = Vec::new();
        let mut index = 0usize;
        let bytes = sql.as_bytes();
        let mut cursor = 0usize;

        while cursor < bytes.len() {
            if bytes[cursor] == b'\'' {
                cursor = push_skipped(&mut transformed, sql, cursor, skip_single_quoted_string);
                continue;
            }
            if bytes[cursor] == b'"' {
                cursor = push_skipped(&mut transformed, sql, cursor, skip_double_quoted_identifier);
                continue;
            }
            if bytes[cursor..].starts_with(b"--") {
                cursor = push_skipped(&mut transformed, sql, cursor, skip_line_comment);
                continue;
            }
            if bytes[cursor..].starts_with(b"/*") {
                cursor = push_skipped(&mut transformed, sql, cursor, skip_block_comment);
                continue;
            }
            if let Some(end) = skip_dollar_quoted_string(sql, cursor) {
                transformed.push_str(&sql[cursor..end]);
                cursor = end;
                continue;
            }
            if bytes[cursor] != b'$' {
                let ch = sql[cursor..].chars().next().unwrap();
                transformed.push(ch);
                cursor += ch.len_utf8();
                continue;
            }

            let start = cursor + 1;
            if start < bytes.len() && bytes[start].is_ascii_digit() {
                let mut end = start + 1;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                return Err(anyhow!(
                    "numeric placeholders are not supported: {}",
                    &sql[cursor..end]
                ));
            }
            if start >= bytes.len() || !is_param_start(bytes[start]) {
                transformed.push('$');
                cursor += 1;
                continue;
            }

            let mut end = start + 1;
            while end < bytes.len() && is_param_continue(bytes[end]) {
                end += 1;
            }

            index += 1;
            params.push(sql[start..end].to_string());
            match dialect_type {
                DialectType::MySql | DialectType::Sqlite => transformed.push('?'),
                DialectType::PostgreSql => {
                    transformed.push('$');
                    transformed.push_str(&index.to_string());
                }
            }
            cursor = end;
        }

        Ok((transformed, params))
    }

    pub fn extract_params(param_names: &[String], json_data: &JsonValue) -> Result<Vec<JsonValue>> {
        let mut values = Vec::new();
        for name in param_names {
            if let Some(val) = json_data.get(name) {
                values.push(val.clone());
            } else {
                return Err(anyhow!("Missing parameter: {}", name));
            }
        }
        Ok(values)
    }

}

fn push_skipped(
    transformed: &mut String,
    sql: &str,
    start: usize,
    skipper: fn(&[u8], usize) -> usize,
) -> usize {
    let end = skipper(sql.as_bytes(), start);
    transformed.push_str(&sql[start..end]);
    end
}

fn skip_single_quoted_string(bytes: &[u8], start: usize) -> usize {
    let mut idx = start + 1;
    while idx < bytes.len() {
        if bytes[idx] == b'\'' {
            if idx + 1 < bytes.len() && bytes[idx + 1] == b'\'' {
                idx += 2;
            } else {
                return idx + 1;
            }
        } else {
            idx += 1;
        }
    }
    bytes.len()
}

fn skip_double_quoted_identifier(bytes: &[u8], start: usize) -> usize {
    let mut idx = start + 1;
    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if idx + 1 < bytes.len() && bytes[idx + 1] == b'"' {
                idx += 2;
            } else {
                return idx + 1;
            }
        } else {
            idx += 1;
        }
    }
    bytes.len()
}

fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|ch| *ch == b'\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(bytes.len())
}

fn skip_block_comment(bytes: &[u8], start: usize) -> usize {
    bytes[start + 2..]
        .windows(2)
        .position(|window| window == b"*/")
        .map(|offset| start + 2 + offset + 2)
        .unwrap_or(bytes.len())
}

fn skip_dollar_quoted_string(sql: &str, start: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    if bytes.get(start) != Some(&b'$') {
        return None;
    }

    let mut tag_end = start + 1;
    while tag_end < bytes.len() && is_dollar_quote_tag(bytes[tag_end]) {
        tag_end += 1;
    }
    if bytes.get(tag_end) != Some(&b'$') {
        return None;
    }

    let delimiter = &sql[start..=tag_end];
    let content_start = tag_end + 1;
    sql[content_start..]
        .find(delimiter)
        .map(|offset| content_start + offset + delimiter.len())
}

fn is_dollar_quote_tag(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

fn is_param_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic() || ch == b'_'
}

fn is_param_continue(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mysql_transformation() {
        let sql = "SELECT * FROM users WHERE id = $id AND name = $name";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::MySql).unwrap();
        assert!(
            transformed
                .to_uppercase()
                .contains("WHERE ID = ? AND NAME = ?")
        );
        assert_eq!(params, vec!["id".to_string(), "name".to_string()]);
    }

    #[test]
    fn test_postgresql_transformation() {
        let sql = "SELECT * FROM users WHERE id = $id AND name = $name";
        let (transformed, params) =
            SqlTransformer::transform(sql, DialectType::PostgreSql).unwrap();
        assert!(
            transformed
                .to_uppercase()
                .contains("WHERE ID = $1 AND NAME = $2")
        );
        assert_eq!(params, vec!["id".to_string(), "name".to_string()]);
    }

    #[test]
    fn test_subquery_transformation() {
        let sql = "SELECT * FROM (SELECT id FROM users WHERE age > $age) WHERE id = $id";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::MySql).unwrap();
        assert!(transformed.to_uppercase().contains("AGE > ?"));
        assert!(transformed.to_uppercase().contains("ID = ?"));
        assert_eq!(params, vec!["age".to_string(), "id".to_string()]);
    }

    #[test]
    fn test_projection_transformation() {
        let sql = "SELECT $id as user_id, name FROM users";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::MySql).unwrap();
        assert!(transformed.to_uppercase().contains("SELECT ? AS USER_ID"));
        assert_eq!(params, vec!["id".to_string()]);
    }

    #[test]
    fn test_security_single_statement() {
        let sql = "SELECT * FROM users; DROP TABLE users;";
        let result = SqlTransformer::transform(sql, DialectType::MySql);
        assert!(result.is_err());
    }

    #[test]
    fn test_dml_statements_are_allowed_for_configured_crud_apis() {
        let sql = "DELETE FROM users WHERE id = $id";
        let result = SqlTransformer::transform(sql, DialectType::MySql);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dml_transformation() {
        let cases = vec![
            (
                "INSERT INTO demo_items (name, status) VALUES ($name, $status)",
                vec!["name", "status"],
            ),
            (
                "UPDATE demo_items SET name = $name WHERE id = $id",
                vec!["name", "id"],
            ),
            ("DELETE FROM demo_items WHERE id = $id", vec!["id"]),
        ];

        for (sql, expected_params) in cases {
            let (transformed, params) = SqlTransformer::transform(sql, DialectType::Sqlite)
                .unwrap_or_else(|_| panic!("Failed to transform: {}", sql));
            assert!(
                transformed.contains("?"),
                "no placeholders in {}",
                transformed
            );
            assert_eq!(params, expected_params, "wrong params for {}", sql);
        }
    }

    #[test]
    fn test_named_params_skip_comments_and_quoted_regions() {
        let sql = "-- $todo\nSELECT \"$quoted\" FROM users /* $block */ WHERE id = $id";
        let (transformed, params) =
            SqlTransformer::replace_named_params(sql, DialectType::PostgreSql).unwrap();

        assert!(transformed.contains("-- $todo"));
        assert!(transformed.contains("\"$quoted\""));
        assert!(transformed.contains("/* $block */"));
        assert!(transformed.contains("WHERE id = $1"));
        assert_eq!(params, vec!["id".to_string()]);
    }

    #[test]
    fn test_named_params_skip_single_and_dollar_quoted_strings() {
        let sql = "SELECT '$literal', 'it''s $escaped', $$ $not_param $$, $tag$ $skip $tag$, $real";
        let (transformed, params) =
            SqlTransformer::replace_named_params(sql, DialectType::PostgreSql).unwrap();

        assert!(transformed.contains("'$literal'"));
        assert!(transformed.contains("'it''s $escaped'"));
        assert!(transformed.contains("$$ $not_param $$"));
        assert!(transformed.contains("$tag$ $skip $tag$"));
        assert!(transformed.ends_with("$1"));
        assert_eq!(params, vec!["real".to_string()]);
    }

    #[test]
    fn test_numeric_placeholders_are_rejected() {
        let error =
            SqlTransformer::transform("SELECT * FROM t WHERE id = $1", DialectType::PostgreSql)
                .unwrap_err()
                .to_string();

        assert!(error.contains("numeric placeholders are not supported"));
    }

    #[test]
    fn test_numeric_placeholder_like_text_is_skipped_in_comments_and_quotes() {
        let sql = "-- $1\nSELECT '$2', \"col_$3\", $$ $4 $$, $tag$ $5 $tag$, $real";
        let (transformed, params) =
            SqlTransformer::replace_named_params(sql, DialectType::PostgreSql).unwrap();

        assert!(transformed.contains("-- $1"));
        assert!(transformed.contains("'$2'"));
        assert!(transformed.contains("\"col_$3\""));
        assert!(transformed.contains("$$ $4 $$"));
        assert!(transformed.contains("$tag$ $5 $tag$"));
        assert!(transformed.ends_with("$1"));
        assert_eq!(params, vec!["real".to_string()]);
    }

    #[test]
    fn test_values_clause_transformation() {
        let sql = "SELECT * FROM (VALUES (1, $id)) AS t(a, b)";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::MySql).unwrap();
        // If this fails to find $id, params will be empty and transformed will still have $id
        assert_eq!(params, vec!["id".to_string()]);
        assert!(transformed.contains("?"));
    }

    #[test]
    fn test_complex_expressions() {
        let cases = vec![
            ("SELECT * FROM x WHERE name LIKE $name", vec!["name"]),
            (
                "SELECT * FROM x WHERE id IN ($id1, $id2)",
                vec!["id1", "id2"],
            ),
            (
                "SELECT CASE WHEN id = $id THEN $val1 ELSE $val2 END FROM x",
                vec!["id", "val1", "val2"],
            ),
            ("SELECT COALESCE(name, $default) FROM x", vec!["default"]),
            (
                "SELECT CAST(id AS VARCHAR) = $id_str FROM x",
                vec!["id_str"],
            ),
            (
                "SELECT * FROM x WHERE id BETWEEN $low AND $high",
                vec!["low", "high"],
            ),
            ("SELECT * FROM x WHERE id IS NULL OR id = $id", vec!["id"]),
            (
                "SELECT EXTRACT(YEAR FROM date_col) = $year FROM x",
                vec!["year"],
            ),
        ];

        for (sql, expected_params) in cases {
            let (_transformed, params) = SqlTransformer::transform(sql, DialectType::MySql)
                .unwrap_or_else(|_| panic!("Failed to transform: {}", sql));
            assert_eq!(params, expected_params, "Failed for SQL: {}", sql);
        }
    }
}
