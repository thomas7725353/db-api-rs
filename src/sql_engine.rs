use sqlparser::dialect::{MySqlDialect, PostgreSqlDialect, SQLiteDialect, Dialect};
use sqlparser::parser::Parser;
use sqlparser::ast::{Statement, Query, SetExpr, Select, Expr, Value, JoinOperator, JoinConstraint, TableFactor, FunctionArg, FunctionArgExpr, SelectItem};
use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;

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

        let mut statements = Parser::parse_sql(&*dialect, sql)?;

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
        if let Some(with) = &mut query.with {
            for cte in &mut with.cte_tables {
                Self::visit_query(&mut cte.query, params, index, dialect);
            }
        }

        match &mut *query.body {
            SetExpr::Select(select) => Self::visit_select(select, params, index, dialect),
            SetExpr::Query(subquery) => Self::visit_query(subquery, params, index, dialect),
            SetExpr::SetOperation { left, op: _, right, set_quantifier: _ } => {
                Self::visit_set_expr(left, params, index, dialect);
                Self::visit_set_expr(right, params, index, dialect);
            }
            _ => {}
        }

        for expr in &mut query.order_by {
            Self::visit_expr(&mut expr.expr, params, index, dialect);
        }
        if let Some(limit) = &mut query.limit {
            Self::visit_expr(limit, params, index, dialect);
        }
        if let Some(offset) = &mut query.offset {
            Self::visit_expr(&mut offset.value, params, index, dialect);
        }
    }

    fn visit_set_expr(set_expr: &mut SetExpr, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        match set_expr {
            SetExpr::Select(select) => Self::visit_select(select, params, index, dialect),
            SetExpr::Query(query) => Self::visit_query(query, params, index, dialect),
            SetExpr::SetOperation { left, op: _, right, set_quantifier: _ } => {
                Self::visit_set_expr(left, params, index, dialect);
                Self::visit_set_expr(right, params, index, dialect);
            }
            _ => {}
        }
    }

    fn visit_select(select: &mut Select, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        for item in &mut select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => Self::visit_expr(expr, params, index, dialect),
                SelectItem::ExprWithAlias { expr, alias: _ } => Self::visit_expr(expr, params, index, dialect),
                _ => {}
            }
        }

        for table in &mut select.from {
            Self::visit_table_factor(&mut table.relation, params, index, dialect);
            for join in &mut table.joins {
                Self::visit_table_factor(&mut join.relation, params, index, dialect);
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

        if let Some(selection) = &mut select.selection {
            Self::visit_expr(selection, params, index, dialect);
        }

        match &mut select.group_by {
            sqlparser::ast::GroupByExpr::Expressions(exprs) => {
                for expr in exprs {
                    Self::visit_expr(expr, params, index, dialect);
                }
            }
            _ => {}
        }

        if let Some(having) = &mut select.having {
            Self::visit_expr(having, params, index, dialect);
        }
    }

    fn visit_table_factor(tf: &mut TableFactor, params: &mut Vec<String>, index: &mut usize, dialect: DialectType) {
        match tf {
            TableFactor::Derived { lateral: _, subquery, alias: _ } => {
                Self::visit_query(subquery, params, index, dialect);
            }
            TableFactor::TableFunction { expr, alias: _ } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            _ => {}
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
            Expr::Identifier(ident) if ident.value.starts_with('$') => {
                let param_name = ident.value[1..].to_string();
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
            Expr::UnaryOp { op: _, expr } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            Expr::Nested(e) => {
                Self::visit_expr(e, params, index, dialect);
            }
            Expr::InList { expr, list, negated: _ } => {
                Self::visit_expr(expr, params, index, dialect);
                for item in list {
                    Self::visit_expr(item, params, index, dialect);
                }
            }
            Expr::InSubquery { expr, subquery, negated: _ } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_query(subquery, params, index, dialect);
            }
            Expr::Between { expr, negated: _, low, high } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_expr(low, params, index, dialect);
                Self::visit_expr(high, params, index, dialect);
            }
            Expr::Case { operand, conditions, results, else_result } => {
                if let Some(op) = operand {
                    Self::visit_expr(op, params, index, dialect);
                }
                for cond in conditions {
                    Self::visit_expr(cond, params, index, dialect);
                }
                for res in results {
                    Self::visit_expr(res, params, index, dialect);
                }
                if let Some(el) = else_result {
                    Self::visit_expr(el, params, index, dialect);
                }
            }
            Expr::Exists { subquery, negated: _ } => {
                Self::visit_query(subquery, params, index, dialect);
            }
            Expr::Subquery(q) => {
                Self::visit_query(q, params, index, dialect);
            }
            Expr::Function(f) => {
                for arg in &mut f.args {
                    match arg {
                        FunctionArg::Unnamed(arg_expr) => {
                            match arg_expr {
                                FunctionArgExpr::Expr(e) => Self::visit_expr(e, params, index, dialect),
                                _ => {}
                            }
                        }
                        FunctionArg::Named { name: _, arg } => {
                            match arg {
                                FunctionArgExpr::Expr(e) => Self::visit_expr(e, params, index, dialect),
                                _ => {}
                            }
                        }
                    }
                }
            }
            Expr::Cast { expr, data_type: _, .. } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            Expr::TypedString { data_type: _, value: _ } => {}
            Expr::CompoundIdentifier(_) => {}
            Expr::Identifier(_) => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_security_select_only() {
        let sql = "DELETE FROM users WHERE id = $id";
        let result = SqlTransformer::transform(sql, DialectType::MySql);
        assert!(result.is_err());
    }
}
