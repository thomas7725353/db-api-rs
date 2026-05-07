use crate::{
    bundle_files,
    cli::McpArgs,
    dbapi_client::DbapiClient,
    manifest::{DraftSqlInput, DraftTableInput, ValidationReport},
    manifest_generator, manifest_validator,
};
use anyhow::Context;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Clone)]
pub struct DbapiMcpServer {
    client: DbapiClient,
    allow_write: bool,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectTableRequest {
    pub datasource_id: String,
    pub table: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidateBundleRequest {
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyBundleRequest {
    pub dir: PathBuf,
    pub allow_write: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyBundleResponse {
    message: String,
    validation_report: ValidationReport,
}

#[tool_router]
impl DbapiMcpServer {
    pub fn new(client: DbapiClient, allow_write: bool) -> Self {
        Self {
            client,
            allow_write,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List datasource definitions from the DBAPI server.")]
    async fn list_datasources(&self) -> Result<CallToolResult, McpError> {
        let datasources = self.client.list_datasources().await.map_err(mcp_internal)?;
        structured(datasources)
    }

    #[tool(description = "Inspect a table schema for a datasource.")]
    async fn inspect_table_schema(
        &self,
        Parameters(req): Parameters<InspectTableRequest>,
    ) -> Result<CallToolResult, McpError> {
        let schema = self
            .client
            .inspect_table_schema(&req.datasource_id, &req.table)
            .await
            .map_err(mcp_internal)?;
        structured(schema)
    }

    #[tool(description = "Draft a table CRUD API bundle without applying it to DBAPI.")]
    async fn draft_table_crud_bundle(
        &self,
        Parameters(req): Parameters<DraftTableInput>,
    ) -> Result<CallToolResult, McpError> {
        let schema = self
            .client
            .inspect_table_schema(&req.datasource_id, &req.table)
            .await
            .map_err(mcp_internal)?;
        let bundle =
            manifest_generator::draft_table_crud_bundle(req, &schema).map_err(mcp_internal)?;
        structured(bundle)
    }

    #[tool(description = "Draft a SQL API bundle without applying it to DBAPI.")]
    async fn draft_sql_api_bundle(
        &self,
        Parameters(req): Parameters<DraftSqlInput>,
    ) -> Result<CallToolResult, McpError> {
        let bundle = manifest_generator::draft_sql_api_bundle(req).map_err(mcp_internal)?;
        structured(bundle)
    }

    #[tool(description = "Validate a local API config bundle directory against the DBAPI server.")]
    async fn validate_api_bundle(
        &self,
        Parameters(req): Parameters<ValidateBundleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let report = validate_bundle_dir(&self.client, &req.dir).await?;
        structured(report)
    }

    #[tool(
        description = "Apply a local API config bundle after validation. Requires process --allow-write and request allowWrite=true."
    )]
    async fn apply_api_config_bundle(
        &self,
        Parameters(req): Parameters<ApplyBundleRequest>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(report) = write_guard_report(self.allow_write, req.allow_write) {
            return structured(report);
        }

        let groups = bundle_files::read_group_file(&req.dir).map_err(mcp_internal)?;
        let api = bundle_files::read_api_file(&req.dir).map_err(mcp_internal)?;
        let report = manifest_validator::validate_against_server(&self.client, &groups, &api)
            .await
            .map_err(mcp_internal)?;
        if !report.success {
            return structured(report);
        }

        self.client
            .import_groups_file(&req.dir.join("api_group_config.json"))
            .await
            .map_err(mcp_internal)?;
        self.client
            .import_api_file(&req.dir.join("api_config.json"))
            .await
            .map_err(mcp_internal)?;

        structured(ApplyBundleResponse {
            message: format!("bundle applied from {}", req.dir.display()),
            validation_report: report,
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DbapiMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "db-api-rs-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Use the DBAPI tools to list datasources, inspect table schemas, draft API bundles, validate bundle files, and apply bundles only when both write gates are enabled.",
            )
    }
}

pub async fn serve(args: McpArgs) -> anyhow::Result<()> {
    if !args.transport.eq_ignore_ascii_case("http") {
        anyhow::bail!(
            "unsupported MCP transport {}; only http is supported",
            args.transport
        );
    }

    let client = DbapiClient::new(args.base_url)?;
    let allow_write = args.allow_write;
    let cancellation_token = CancellationToken::new();
    let shutdown_token = cancellation_token.clone();
    let service: StreamableHttpService<DbapiMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(DbapiMcpServer::new(client.clone(), allow_write)),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_sse_keep_alive(None)
                .with_cancellation_token(cancellation_token.child_token()),
        );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&args.listen)
        .await
        .with_context(|| format!("binding MCP HTTP listener on {}", args.listen))?;

    info!("db-api-rs MCP sidecar listening on {}", args.listen);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            if let Err(error) = tokio::signal::ctrl_c().await {
                tracing::warn!("failed waiting for shutdown signal: {error}");
            }
            shutdown_token.cancel();
        })
        .await?;
    Ok(())
}

async fn validate_bundle_dir(
    client: &DbapiClient,
    dir: &PathBuf,
) -> Result<ValidationReport, McpError> {
    let groups = bundle_files::read_group_file(dir).map_err(mcp_internal)?;
    let api = bundle_files::read_api_file(dir).map_err(mcp_internal)?;
    manifest_validator::validate_against_server(client, &groups, &api)
        .await
        .map_err(mcp_internal)
}

fn write_guard_report(
    process_allow_write: bool,
    request_allow_write: bool,
) -> Option<ValidationReport> {
    if process_allow_write && request_allow_write {
        return None;
    }

    let mut report = ValidationReport::default();
    report.error("apply_api_config_bundle requires process --allow-write and tool allowWrite=true");
    Some(report)
}

fn structured(value: impl Serialize) -> Result<CallToolResult, McpError> {
    let value = serde_json::to_value(value).map_err(mcp_internal)?;
    Ok(CallToolResult::structured(value))
}

fn mcp_internal(error: impl Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_guard_requires_process_and_request_allow_write() {
        for (process_allow_write, request_allow_write) in
            [(false, false), (false, true), (true, false)]
        {
            let report =
                write_guard_report(process_allow_write, request_allow_write).expect("blocked");
            assert!(!report.success);
            assert_eq!(report.errors.len(), 1);
            assert!(report.errors[0].contains("--allow-write"));
            assert!(report.errors[0].contains("allowWrite=true"));
        }

        assert!(write_guard_report(true, true).is_none());
    }
}
