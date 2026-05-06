use anyhow::{Result, anyhow};
use serde_json::Value as JsonValue;
use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, JoinConstraint, JoinOperator, Query, Select, SelectItem,
    SetExpr, Statement, TableFactor, Value,
};
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
        let mut chars = sql.chars().peekable();
        let mut in_single_quote = false;

        while let Some(ch) = chars.next() {
            if ch == '\'' {
                transformed.push(ch);
                in_single_quote = !in_single_quote;
                continue;
            }

            if !in_single_quote && ch == '$' {
                let mut name = String::new();
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        name.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if name.is_empty() {
                    transformed.push(ch);
                    continue;
                }

                index += 1;
                params.push(name);
                match dialect_type {
                    DialectType::MySql | DialectType::Sqlite => transformed.push('?'),
                    DialectType::PostgreSql => {
                        transformed.push('$');
                        transformed.push_str(&index.to_string());
                    }
                }
                continue;
            }

            transformed.push(ch);
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

    #[allow(dead_code)]
    fn visit_statement(
        stmt: &mut Statement,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
        if let Statement::Query(query) = stmt {
            Self::visit_query(query, params, index, dialect);
        }
    }

    #[allow(dead_code)]
    fn visit_query(
        query: &mut Query,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
        if let Some(with) = &mut query.with {
            for cte in &mut with.cte_tables {
                Self::visit_query(&mut cte.query, params, index, dialect);
            }
        }

        Self::visit_set_expr(&mut query.body, params, index, dialect);

        for expr in &mut query.order_by {
            Self::visit_expr(&mut expr.expr, params, index, dialect);
        }
        if let Some(limit) = &mut query.limit {
            Self::visit_expr(limit, params, index, dialect);
        }
        if let Some(offset) = &mut query.offset {
            Self::visit_expr(&mut offset.value, params, index, dialect);
        }
        if let Some(fetch) = &mut query.fetch
            && let Some(quantity) = &mut fetch.quantity
        {
            Self::visit_expr(quantity, params, index, dialect);
        }
    }

    #[allow(dead_code)]
    fn visit_set_expr(
        set_expr: &mut SetExpr,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
        match set_expr {
            SetExpr::Select(select) => Self::visit_select(select, params, index, dialect),
            SetExpr::Query(query) => Self::visit_query(query, params, index, dialect),
            SetExpr::SetOperation {
                left,
                op: _,
                right,
                set_quantifier: _,
            } => {
                Self::visit_set_expr(left, params, index, dialect);
                Self::visit_set_expr(right, params, index, dialect);
            }
            SetExpr::Values(values) => {
                for row in &mut values.rows {
                    for expr in row {
                        Self::visit_expr(expr, params, index, dialect);
                    }
                }
            }
            _ => {}
        }
    }

    #[allow(dead_code)]
    fn visit_select(
        select: &mut Select,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
        if let Some(sqlparser::ast::Distinct::On(exprs)) = &mut select.distinct {
            for expr in exprs {
                Self::visit_expr(expr, params, index, dialect);
            }
        }

        for item in &mut select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => Self::visit_expr(expr, params, index, dialect),
                SelectItem::ExprWithAlias { expr, alias: _ } => {
                    Self::visit_expr(expr, params, index, dialect)
                }
                _ => {}
            }
        }

        for table in &mut select.from {
            Self::visit_table_factor(&mut table.relation, params, index, dialect);
            for join in &mut table.joins {
                Self::visit_table_factor(&mut join.relation, params, index, dialect);
                match &mut join.join_operator {
                    JoinOperator::Inner(constraint)
                    | JoinOperator::LeftOuter(constraint)
                    | JoinOperator::RightOuter(constraint)
                    | JoinOperator::FullOuter(constraint) => {
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

        if let sqlparser::ast::GroupByExpr::Expressions(exprs) = &mut select.group_by {
            for expr in exprs {
                Self::visit_expr(expr, params, index, dialect);
            }
        }

        if let Some(having) = &mut select.having {
            Self::visit_expr(having, params, index, dialect);
        }
    }

    #[allow(dead_code)]
    fn visit_table_factor(
        tf: &mut TableFactor,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
        match tf {
            TableFactor::Derived {
                lateral: _,
                subquery,
                alias: _,
            } => {
                Self::visit_query(subquery, params, index, dialect);
            }
            TableFactor::TableFunction { expr, alias: _ } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            TableFactor::UNNEST { array_exprs, .. } => {
                for expr in array_exprs {
                    Self::visit_expr(expr, params, index, dialect);
                }
            }
            TableFactor::NestedJoin {
                table_with_joins,
                alias: _,
            } => {
                Self::visit_table_factor(&mut table_with_joins.relation, params, index, dialect);
                for join in &mut table_with_joins.joins {
                    Self::visit_table_factor(&mut join.relation, params, index, dialect);
                    match &mut join.join_operator {
                        JoinOperator::Inner(JoinConstraint::On(expr))
                        | JoinOperator::LeftOuter(JoinConstraint::On(expr))
                        | JoinOperator::RightOuter(JoinConstraint::On(expr))
                        | JoinOperator::FullOuter(JoinConstraint::On(expr)) => {
                            Self::visit_expr(expr, params, index, dialect);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    #[allow(dead_code)]
    fn visit_expr(
        expr: &mut Expr,
        params: &mut Vec<String>,
        index: &mut usize,
        dialect: DialectType,
    ) {
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
            Expr::InList {
                expr,
                list,
                negated: _,
            } => {
                Self::visit_expr(expr, params, index, dialect);
                for item in list {
                    Self::visit_expr(item, params, index, dialect);
                }
            }
            Expr::InSubquery {
                expr,
                subquery,
                negated: _,
            } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_query(subquery, params, index, dialect);
            }
            Expr::Between {
                expr,
                negated: _,
                low,
                high,
            } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_expr(low, params, index, dialect);
                Self::visit_expr(high, params, index, dialect);
            }
            Expr::Case {
                operand,
                conditions,
                results,
                else_result,
            } => {
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
            Expr::Exists {
                subquery,
                negated: _,
            } => {
                Self::visit_query(subquery, params, index, dialect);
            }
            Expr::Subquery(q) => {
                Self::visit_query(q, params, index, dialect);
            }
            Expr::Function(f) => {
                for arg in &mut f.args {
                    match arg {
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => {
                            Self::visit_expr(e, params, index, dialect)
                        }
                        FunctionArg::Named {
                            name: _,
                            arg: FunctionArgExpr::Expr(e),
                        } => Self::visit_expr(e, params, index, dialect),
                        _ => {}
                    }
                }
                if let Some(over) = &mut f.over {
                    match over {
                        sqlparser::ast::WindowType::WindowSpec(spec) => {
                            for expr in &mut spec.partition_by {
                                Self::visit_expr(expr, params, index, dialect);
                            }
                            for order_by in &mut spec.order_by {
                                Self::visit_expr(&mut order_by.expr, params, index, dialect);
                            }
                        }
                        sqlparser::ast::WindowType::NamedWindow(_) => {}
                    }
                }
                if let Some(filter) = &mut f.filter {
                    Self::visit_expr(filter, params, index, dialect);
                }
            }
            Expr::Cast {
                expr, data_type: _, ..
            } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            Expr::Extract { field: _, expr } => {
                Self::visit_expr(expr, params, index, dialect);
            }
            Expr::IsNull(expr)
            | Expr::IsNotNull(expr)
            | Expr::IsTrue(expr)
            | Expr::IsFalse(expr)
            | Expr::IsUnknown(expr)
            | Expr::IsNotTrue(expr)
            | Expr::IsNotFalse(expr)
            | Expr::IsNotUnknown(expr) => {
                Self::visit_expr(expr, params, index, dialect);
            }
            Expr::Like { expr, pattern, .. }
            | Expr::ILike { expr, pattern, .. }
            | Expr::SimilarTo { expr, pattern, .. } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_expr(pattern, params, index, dialect);
            }
            Expr::Trim {
                expr, trim_what, ..
            } => {
                Self::visit_expr(expr, params, index, dialect);
                if let Some(tw) = trim_what {
                    Self::visit_expr(tw, params, index, dialect);
                }
            }
            Expr::Substring {
                expr,
                substring_from,
                substring_for,
                ..
            } => {
                Self::visit_expr(expr, params, index, dialect);
                if let Some(from) = substring_from {
                    Self::visit_expr(from, params, index, dialect);
                }
                if let Some(to) = substring_for {
                    Self::visit_expr(to, params, index, dialect);
                }
            }
            Expr::Overlay {
                expr,
                overlay_what,
                overlay_from,
                overlay_for,
            } => {
                Self::visit_expr(expr, params, index, dialect);
                Self::visit_expr(overlay_what, params, index, dialect);
                Self::visit_expr(overlay_from, params, index, dialect);
                if let Some(of) = overlay_for {
                    Self::visit_expr(of, params, index, dialect);
                }
            }
            Expr::Array(a) => {
                for expr in &mut a.elem {
                    Self::visit_expr(expr, params, index, dialect);
                }
            }
            Expr::ListAgg(l) => {
                Self::visit_expr(&mut l.expr, params, index, dialect);
            }
            Expr::ArrayAgg(a) => {
                Self::visit_expr(&mut a.expr, params, index, dialect);
            }
            Expr::GroupingSets(groups) => {
                for group in groups {
                    for expr in group {
                        Self::visit_expr(expr, params, index, dialect);
                    }
                }
            }
            Expr::Cube(groups) => {
                for group in groups {
                    for expr in group {
                        Self::visit_expr(expr, params, index, dialect);
                    }
                }
            }
            Expr::Rollup(groups) => {
                for group in groups {
                    for expr in group {
                        Self::visit_expr(expr, params, index, dialect);
                    }
                }
            }
            Expr::TypedString {
                data_type: _,
                value: _,
            } => {}
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
