# Design Spec: db-api Rust Refactor (Single Machine)

## 1. Overview
Refactor the existing Java-based `db-api` project into a high-performance, memory-safe Rust implementation. The goal is to provide a dynamic API gateway that turns SQL queries into REST/gRPC endpoints with minimal configuration, supporting multiple backend databases.

## 2. Goals
- **Single-machine performance**: Significant reduction in memory footprint and improved request latency.
- **Dynamic Data Sources**: Support for MySQL, PostgreSQL, SQLite, and DuckDB.
- **SQL-to-API**: Automatically map SQL queries with parameters (e.g., `$param`) to API endpoints.
- **Metadata Management**: Maintain API and data source configurations in a local SQLite database.
- **Caching**: High-performance local caching for configurations and optionally for query results.

## 3. Technology Stack
## 3. Technology Stack
- **Web/API**: `axum` (standard, high-performance REST framework).
- **ORM/Metadata**: `rbatis` with `rbdc-sqlite` (for managing `data.db`).
- **Dynamic Execution**: `rbdc` drivers (directly or via `rbatis` instances) for MySQL, PostgreSQL, and SQLite.
- **SQL Parsing**: `sqlparser-rs` (v0.43+) for placeholder identification and dialect-specific transformation.
- **Metadata Storage**: `SQLite` (maintaining compatibility with the existing `data.db` schema).
- **Caching**: `moka` (for API configurations).
- **Runtime**: `tokio`.

## 4. Architecture

### 4.1 Metadata Layer & Compatibility
- **Existing `data.db`**: The Rust version MUST use the same table structures (`api_config`, `datasource`) to ensure the existing Java management UI (if still used) or the database remains functional.
- **Config Service**: 
    - Loads configurations into memory.
    - Uses `moka` to cache `ApiConfig` by path.
    - Status control (online/offline) logic from the Java version will be replicated.

### 4.2 Connection Management (Multi-Instance)
- **Dynamic Pool Manager**:
    - A `DashMap<i32, RBatis>` where each entry is a fully configured `RBatis` instance dedicated to one data source. This respects RBatis's architecture of one driver type per instance.
    - Supports `mysql`, `postgres`, `sqlite` in Phase 1.

### 4.3 Execution Pipeline (Security First)
1.  **Request Handling**: `axum` receives a REST request at `/api/:path`.
2.  **Configuration Lookup**: Fetch `ApiConfig` from `moka` cache.
3.  **SQL Pre-processing**:
    - `sqlparser-rs` parses the SQL template.
    - Identifies `$param` placeholders.
    - **Security Fix**: Instead of string replacement, the engine converts `$name` to bind placeholders (`?` or `$1`) and extracts values into a `Vec<rbdc::Value>`.
4.  **Execution Constraints**: 
    - Enforce read-only (SELECT) checks via the parser.
    - Implement optional row limits and timeouts.
5.  **Execution**: Execute via Prepared Statement using the target `RBatis` instance.
6.  **Response**: Stream results as JSON.

## 7. Security Considerations
- **No String Concatenation**: The Java version's `sql.replace` logic is replaced by strict Parameter Binding.
- **Dialect Guarding**: Use the correct `sqlparser` Dialect to prevent cross-dialect syntax exploits.
- **Query Validation**: Reject multi-statement queries or non-SELECT queries unless explicitly allowed by configuration.

- `ApiConfig`: ID, Path, Name, DataSourceID, SQL Template, Parameters (JSON), Status.

### 5.2 SQL Transformation Logic
- A utility module using `sqlparser-rs` to:
    - Validate SQL syntax against the target dialect.
    - Extract parameter names.
    - Perform surgical replacement of `$name` with indexed/anonymous placeholders.

## 6. Testing Strategy
- **Unit Tests**: SQL transformation logic, parameter extraction, cache behavior.
- **Integration Tests**: End-to-end flow from HTTP request to mock/local database execution.
- **Benchmarks**: Compare memory usage and latency against the original Java implementation.

## 7. Security Considerations
- **SQL Injection Prevention**: Using `sqlparser-rs` to ensure only parameterized queries are executed via Prepared Statements.
- **Access Control**: (Optional for Phase 1) Basic API Key or Token-based authentication.
