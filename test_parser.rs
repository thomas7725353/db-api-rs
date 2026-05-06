use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::{Statement, SetExpr};

#[test]
fn test_values_clause() {
    let sql = "SELECT * FROM (VALUES (1, $id)) AS t(a, b)";
    let dialect = MySqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql).unwrap();
    println!("{:#?}", statements[0]);
}
