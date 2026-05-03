use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

fn main() {
    let sql = "SELECT * FROM users WHERE id = $id";
    let dialect = GenericDialect {};
    match Parser::parse_sql(&dialect, sql) {
        Ok(ast) => println!("Success: {:?}", ast),
        Err(e) => println!("Error: {:?}", e),
    }
}
