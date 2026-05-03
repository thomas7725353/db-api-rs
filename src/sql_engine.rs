use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::{Statement, Query, SetExpr, Select, Expr, Value, JoinOperator, JoinConstraint};
use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy)]
pub enum DialectType {
    MySql,
    PostgreSql,
    Sqlite,
}

pub struct SqlTransformer;

impl SqlTransformer {
    pub fn transform(sql: &str, dialect_type: DialectType) -> Result<(String, Vec<String>)> {
        let dialect = GenericDialect {};
        let mut statements = Parser::parse_sql(&dialect, sql)?;

        if statements.len() != 1 {
            return Err(anyhow!("Only single statement allowed"));
        }

        let statement = &mut statements[0];

        // Security Guard: Only SELECT allowed
        match statement {
            Statement::Query(_) => {},
            _ => return Err(anyhow!("Only SELECT statements are allowed")),
        }

        let mut params = Vec::new();
        let mut placeholder_index = 0;

        // Traverse AST to find and replace placeholders
        Self::visit_statement(statement, &mut params, &mut placeholder_index, dialect_type);

        let transformed_sql = statement.to_string();

        Ok((transformed_sql, params))
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

    fn visit_statement(stmt: &mut Statement, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        if let Statement::Query(query) = stmt {
            Self::visit_query(query, params, index, dialect);
        }
    }

    fn visit_query(query: &mut Query, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        if let SetExpr::Select(select) = &mut *query.body {
            Self::visit_select(select, params, index, dialect);
        }
        for expr in &mut query.order_by {
            Self::visit_expr(&mut expr.expr, params, index, dialect);
        }
        if let Some(limit) = &mut query.limit {
            Self::visit_expr(limit, params, index, dialect);
        }
    }

    fn visit_select(select: &mut Select, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        if let Some(selection) = &mut select.selection {
            Self::visit_expr(selection, params, index, dialect);
        }
        for table in &mut select.from {
            for join in &mut table.joins {
                match &mut join.join_operator {
                    JoinOperator::Inner(constraint) |
                    JoinOperator::LeftOuter(constraint) |
                    JoinOperator::RightOuter(constraint) |
                    JoinOperator::FullOuter(constraint) => {
                        if let JoinConstraint::On(expr) = constraint {
                            Self::visit_expr(expr, params, index, dialect);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn visit_expr(expr: &mut Expr, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        match expr {
            Expr::Value(Value::Placeholder(name)) if name.starts_with('$') => {
                let param_name = name[1..].to_string();
                params.push(param_name);
                *index += 1;
                match dialect {
                    DialectType::MySql | DialectType::Sqlite => {
                        *expr = Expr::Value(Value::Placeholder("?".to_string()));
                    }
                    DialectType::PostgreSql => {
                        *expr = Expr::Value(Value::Placeholder(format!("${}", index)));
                    }
                }
            }
            Expr::BinaryOp { left, op: _, right } => {
                Self::visit_expr(left, params, index, dialect);
                Self::visit_expr(right, params, index, dialect);
            }
            Expr::InList { expr, list, negated: _ } => {
                Self::visit_expr(expr, params, index, dialect);
                for item in list {
                    Self::visit_expr(item, params, index, dialect);
                }
            }
            Expr::CompoundIdentifier(_) => {}
            Expr::Identifier(_) => {}
            Expr::Function(f) => {
                for arg in &mut f.args {
                    match arg {
                        sqlparser::ast::FunctionArg::Unnamed(arg_expr) => {
                            match arg_expr {
                                sqlparser::ast::FunctionArgExpr::Expr(e) => Self::visit_expr(e, params, index, dialect),
                                _ => {}
                            }
                        }
                        sqlparser::ast::FunctionArg::Named { name: _, arg } => {
                            match arg {
                                sqlparser::ast::FunctionArgExpr::Expr(e) => Self::visit_expr(e, params, index, dialect),
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mysql_transformation() {
        let sql = "SELECT * FROM users WHERE id = $id AND name = $name";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::MySql).unwrap();
        assert!(transformed.to_uppercase().contains("WHERE ID = ? AND NAME = ?"));
        assert_eq!(params, vec!["id".to_string(), "name".to_string()]);
    }

    #[test]
    fn test_postgresql_transformation() {
        let sql = "SELECT * FROM users WHERE id = $id AND name = $name";
        let (transformed, params) = SqlTransformer::transform(sql, DialectType::PostgreSql).unwrap();
        assert!(transformed.to_uppercase().contains("WHERE ID = $1 AND NAME = $2"));
        assert_eq!(params, vec!["id".to_string(), "name".to_string()]);
    }

    #[test]
    fn test_security_single_statement() {
        let sql = "SELECT * FROM users; DROP TABLE users;";
        let result = SqlTransformer::transform(sql, DialectType::MySql);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Only single statement allowed"));
    }

    #[test]
    fn test_security_select_only() {
        let sql = "DELETE FROM users WHERE id = $id";
        let result = SqlTransformer::transform(sql, DialectType::MySql);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Only SELECT statements are allowed"));
    }

    #[test]
    fn test_extract_params() {
        let param_names = vec!["id".to_string(), "name".to_string()];
        let json_data = json!({"id": 1, "name": "Alice", "extra": "ignored"});
        let values = SqlTransformer::extract_params(&param_names, &json_data).unwrap();
        assert_eq!(values, vec![json!(1), json!("Alice")]);
    }

    #[test]
    fn test_extract_params_missing() {
        let param_names = vec!["id".to_string(), "missing".to_string()];
        let json_data = json!({"id": 1});
        let result = SqlTransformer::extract_params(&param_names, &json_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing parameter: missing"));
    }
}
