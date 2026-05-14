use crate::{
    bundle_files,
    cli::McpArgs,
    dbapi_client::{DbapiClient, PublishedApiCall},
    manifest::{DraftSqlInput, DraftTableInput, ValidationReport},
    manifest_generator, manifest_validator,
};
use anyhow::Context;
use rmcp::ServiceExt;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        AnnotateAble, CallToolResult, GetPromptRequestParams, GetPromptResult, Implementation,
        ListPromptsResult, ListResourcesResult, PaginatedRequestParams, Prompt, PromptMessage,
        PromptMessageRole, RawResource, ReadResourceRequestParams, ReadResourceResult,
        ResourceContents, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
pub struct ListTablesRequest {
    pub datasource_id: String,
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

const RESOURCE_QUICKSTART: &str = "dbapi://docs/quickstart";
const RESOURCE_BUNDLE_WORKFLOW: &str = "dbapi://docs/bundle-workflow";
const RESOURCE_API_CATALOG: &str = "dbapi://api-catalog";
const RESOURCE_DATASOURCES: &str = "dbapi://datasources";
const RESOURCE_SKILLS: &str = "dbapi://skills";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppTokenRequest {
    pub app_name: String,
    pub group_id: String,
    pub allow_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallPublishedApiRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub authorization: Option<String>,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
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

    #[tool(description = "Check whether the DBAPI HTTP server is reachable.")]
    async fn health_check(&self) -> Result<CallToolResult, McpError> {
        let health = self.client.health_check().await.map_err(mcp_internal)?;
        structured(health)
    }

    #[tool(description = "List datasource definitions from the DBAPI server.")]
    async fn list_datasources(&self) -> Result<CallToolResult, McpError> {
        let datasources = self.client.list_datasources().await.map_err(mcp_internal)?;
        structured(datasources)
    }

    #[tool(description = "List API groups from the DBAPI server.")]
    async fn list_groups(&self) -> Result<CallToolResult, McpError> {
        let groups = self.client.list_groups().await.map_err(mcp_internal)?;
        structured(groups)
    }

    #[tool(description = "List API configs from the DBAPI server.")]
    async fn list_api_configs(&self) -> Result<CallToolResult, McpError> {
        let api_configs = self.client.list_api_configs().await.map_err(mcp_internal)?;
        structured(api_configs)
    }

    #[tool(description = "List table names for a datasource.")]
    async fn list_tables(
        &self,
        Parameters(req): Parameters<ListTablesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let tables = self
            .client
            .list_tables(&req.datasource_id)
            .await
            .map_err(mcp_internal)?;
        structured(tables)
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

    #[tool(description = "Create a DBAPI app, authorize it for a group, and generate a token.")]
    async fn create_app_token_for_group(
        &self,
        Parameters(req): Parameters<CreateAppTokenRequest>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(report) = write_guard_report(self.allow_write, req.allow_write) {
            return structured(report);
        }

        let token = self
            .client
            .create_app_token_for_group(&req.app_name, &req.group_id)
            .await
            .map_err(mcp_internal)?;
        structured(token)
    }

    #[tool(description = "Call a published DBAPI endpoint under /api/{path} for smoke testing.")]
    async fn call_published_api(
        &self,
        Parameters(req): Parameters<CallPublishedApiRequest>,
    ) -> Result<CallToolResult, McpError> {
        if !req.method.eq_ignore_ascii_case("GET")
            && let Some(report) = write_guard_report(self.allow_write, req.allow_write)
        {
            return structured(report);
        }

        let response = self
            .client
            .call_published_api(PublishedApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                authorization: req.authorization,
                headers: req.headers,
            })
            .await
            .map_err(mcp_internal)?;
        structured(response)
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

pub fn inspection_json() -> serde_json::Value {
    json!({
        "name": "db-api-rs-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": {
            "default": "http",
            "supported": ["http", "stdio"],
            "endpoint": "/mcp"
        },
        "cli": {
            "inspect": "db-api-rs mcp inspect --json",
            "call": "db-api-rs mcp --base-url http://127.0.0.1:8520 call <tool> --args-json '{...}'",
            "qaSmoke": "db-api-rs qa smoke --base-url http://127.0.0.1:8520"
        },
        "tools": [
            {
                "name": "health_check",
                "description": "Check whether the DBAPI HTTP server is reachable.",
                "write": false
            },
            {
                "name": "list_datasources",
                "description": "List datasource definitions from the DBAPI server.",
                "write": false
            },
            {
                "name": "list_groups",
                "description": "List API groups from the DBAPI server.",
                "write": false
            },
            {
                "name": "list_api_configs",
                "description": "List API configs from the DBAPI server.",
                "write": false
            },
            {
                "name": "list_tables",
                "description": "List table names for a datasource.",
                "write": false,
                "input": {"datasourceId": "string"}
            },
            {
                "name": "inspect_table_schema",
                "description": "Inspect a table schema for a datasource.",
                "write": false,
                "input": {"datasourceId": "string", "table": "string"}
            },
            {
                "name": "draft_table_crud_bundle",
                "description": "Draft a table CRUD API bundle without applying it to DBAPI.",
                "write": false
            },
            {
                "name": "draft_sql_api_bundle",
                "description": "Draft a SQL API bundle without applying it to DBAPI.",
                "write": false
            },
            {
                "name": "create_app_token_for_group",
                "description": "Create a DBAPI app, authorize it for a group, and generate a token.",
                "write": true,
                "requires": ["process --allow-write", "request allowWrite=true"],
                "input": {"appName": "string", "groupId": "string", "allowWrite": "boolean"}
            },
            {
                "name": "call_published_api",
                "description": "Call a published DBAPI endpoint under /api/{path} for smoke testing.",
                "write": "method-dependent",
                "requires": ["process --allow-write and request allowWrite=true for non-GET calls"],
                "input": {"method": "GET|POST|PUT|PATCH|DELETE", "path": "string", "params": "object", "authorization": "optional string", "allowWrite": "boolean"}
            },
            {
                "name": "validate_api_bundle",
                "description": "Validate a local API config bundle directory against the DBAPI server.",
                "write": false,
                "input": {"dir": "path"}
            },
            {
                "name": "apply_api_config_bundle",
                "description": "Apply a local API config bundle after validation.",
                "write": true,
                "requires": ["process --allow-write", "request allowWrite=true"],
                "input": {"dir": "path", "allowWrite": "boolean"}
            }
        ],
        "resources": [
            "dbapi://docs/quickstart",
            "dbapi://docs/bundle-workflow",
            "dbapi://api-catalog",
            "dbapi://datasources",
            "dbapi://skills"
        ],
        "prompts": [
            "generate_table_api_bundle",
            "generate_sql_api_bundle",
            "review_bundle_before_apply",
            "qa_smoke_test_plan"
        ]
    })
}

pub async fn call_local_tool(
    base_url: String,
    process_allow_write: bool,
    tool_name: &str,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let client = DbapiClient::new(base_url)?;
    match tool_name {
        "health_check" => client.health_check().await,
        "list_datasources" => {
            serde_json::to_value(client.list_datasources().await?).map_err(anyhow::Error::from)
        }
        "list_groups" => {
            serde_json::to_value(client.list_groups().await?).map_err(anyhow::Error::from)
        }
        "list_api_configs" => {
            serde_json::to_value(client.list_api_configs().await?).map_err(anyhow::Error::from)
        }
        "list_tables" => {
            let req: ListTablesRequest = serde_json::from_value(args)?;
            serde_json::to_value(client.list_tables(&req.datasource_id).await?)
                .map_err(anyhow::Error::from)
        }
        "inspect_table_schema" => {
            let req: InspectTableRequest = serde_json::from_value(args)?;
            serde_json::to_value(
                client
                    .inspect_table_schema(&req.datasource_id, &req.table)
                    .await?,
            )
            .map_err(anyhow::Error::from)
        }
        "draft_table_crud_bundle" => {
            let req: DraftTableInput = serde_json::from_value(args)?;
            let schema = client
                .inspect_table_schema(&req.datasource_id, &req.table)
                .await?;
            serde_json::to_value(manifest_generator::draft_table_crud_bundle(req, &schema)?)
                .map_err(anyhow::Error::from)
        }
        "draft_sql_api_bundle" => {
            let req: DraftSqlInput = serde_json::from_value(args)?;
            serde_json::to_value(manifest_generator::draft_sql_api_bundle(req)?)
                .map_err(anyhow::Error::from)
        }
        "create_app_token_for_group" => {
            let req: CreateAppTokenRequest = serde_json::from_value(args)?;
            if let Some(report) = write_guard_report(process_allow_write, req.allow_write) {
                return serde_json::to_value(report).map_err(anyhow::Error::from);
            }
            client
                .create_app_token_for_group(&req.app_name, &req.group_id)
                .await
        }
        "call_published_api" => {
            let req: CallPublishedApiRequest = serde_json::from_value(args)?;
            if !req.method.eq_ignore_ascii_case("GET")
                && let Some(report) = write_guard_report(process_allow_write, req.allow_write)
            {
                return serde_json::to_value(report).map_err(anyhow::Error::from);
            }
            serde_json::to_value(
                client
                    .call_published_api(PublishedApiCall {
                        method: req.method,
                        path: req.path,
                        params: req.params,
                        authorization: req.authorization,
                        headers: req.headers,
                    })
                    .await?,
            )
            .map_err(anyhow::Error::from)
        }
        "validate_api_bundle" => {
            let req: ValidateBundleRequest = serde_json::from_value(args)?;
            serde_json::to_value(validate_bundle_dir(&client, &req.dir).await?)
                .map_err(anyhow::Error::from)
        }
        "apply_api_config_bundle" => {
            let req: ApplyBundleRequest = serde_json::from_value(args)?;
            if let Some(report) = write_guard_report(process_allow_write, req.allow_write) {
                return serde_json::to_value(report).map_err(anyhow::Error::from);
            }

            let groups = bundle_files::read_group_file(&req.dir)?;
            let api = bundle_files::read_api_file(&req.dir)?;
            let report =
                manifest_validator::validate_against_server(&client, &groups, &api).await?;
            if !report.success {
                return serde_json::to_value(report).map_err(anyhow::Error::from);
            }

            client
                .import_groups_file(&req.dir.join("api_group_config.json"))
                .await?;
            client
                .import_api_file(&req.dir.join("api_config.json"))
                .await?;
            serde_json::to_value(ApplyBundleResponse {
                message: format!("bundle applied from {}", req.dir.display()),
                validation_report: report,
            })
            .map_err(anyhow::Error::from)
        }
        other => anyhow::bail!("unknown MCP tool: {other}"),
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DbapiMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
            .with_server_info(Implementation::new(
                "db-api-rs-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Use the DBAPI tools to list datasources, inspect table schemas, draft API bundles, validate bundle files, and apply bundles only when both write gates are enabled.",
            )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            resource(
                RESOURCE_QUICKSTART,
                "DBAPI Quick Start",
                "Runtime, CLI, MCP, and QA smoke quick start.",
                "text/markdown",
            ),
            resource(
                RESOURCE_BUNDLE_WORKFLOW,
                "DBAPI Bundle Workflow",
                "Bundle generation, validation, apply, and verification workflow.",
                "text/markdown",
            ),
            resource(
                RESOURCE_API_CATALOG,
                "DBAPI API Catalog",
                "Live API configuration catalog from the DBAPI server.",
                "application/json",
            ),
            resource(
                RESOURCE_DATASOURCES,
                "DBAPI Datasources",
                "Live datasource list from the DBAPI server.",
                "application/json",
            ),
            resource(
                RESOURCE_SKILLS,
                "DBAPI Skills",
                "Repo-local skill catalog for agent workflows.",
                "application/json",
            ),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let content = match request.uri.as_str() {
            RESOURCE_QUICKSTART => {
                ResourceContents::text(include_str!("../README.md"), RESOURCE_QUICKSTART)
                    .with_mime_type("text/markdown")
            }
            RESOURCE_BUNDLE_WORKFLOW => {
                ResourceContents::text(include_str!("../docs/index.md"), RESOURCE_BUNDLE_WORKFLOW)
                    .with_mime_type("text/markdown")
            }
            RESOURCE_API_CATALOG => {
                let api_configs = self.client.list_api_configs().await.map_err(mcp_internal)?;
                ResourceContents::text(
                    serde_json::to_string_pretty(&api_configs).map_err(mcp_internal)?,
                    RESOURCE_API_CATALOG,
                )
                .with_mime_type("application/json")
            }
            RESOURCE_DATASOURCES => {
                let datasources = self.client.list_datasources().await.map_err(mcp_internal)?;
                ResourceContents::text(
                    serde_json::to_string_pretty(&datasources).map_err(mcp_internal)?,
                    RESOURCE_DATASOURCES,
                )
                .with_mime_type("application/json")
            }
            RESOURCE_SKILLS => {
                ResourceContents::text(include_str!("../skills/index.json"), RESOURCE_SKILLS)
                    .with_mime_type("application/json")
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown resource URI: {other}"),
                    None,
                ));
            }
        };
        Ok(ReadResourceResult::new(vec![content]))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new(
                "generate_table_api_bundle",
                Some("Generate a reviewable table CRUD/list/view API bundle."),
                None,
            ),
            Prompt::new(
                "generate_sql_api_bundle",
                Some("Generate a reviewable single SQL or View SQL API bundle."),
                None,
            ),
            Prompt::new(
                "review_bundle_before_apply",
                Some("Review generated DBAPI bundle files before applying them."),
                None,
            ),
            Prompt::new(
                "qa_smoke_test_plan",
                Some("Create a QA smoke test plan for a DBAPI deployment."),
                None,
            ),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let text = match request.name.as_str() {
            "generate_table_api_bundle" => {
                "Generate a DBAPI table API bundle. First read dbapi://docs/bundle-workflow, then use list_datasources, list_tables, inspect_table_schema, and draft_table_crud_bundle. Keep resourcePath explicit and do not apply the bundle without review."
            }
            "generate_sql_api_bundle" => {
                "Generate a DBAPI SQL API bundle. Use named $params, choose sql or viewSql, call draft_sql_api_bundle, and return the generated bundle for review before any apply step."
            }
            "review_bundle_before_apply" => {
                "Review dbapi_manifest.json, api_group_config.json, api_config.json, curl.md, and VERIFY.md. Then call validate_api_bundle. Only call apply_api_config_bundle when write gates are enabled and the user has approved."
            }
            "qa_smoke_test_plan" => {
                "Create a QA smoke plan for DBAPI. Check health_check, list datasources/groups/API configs, validate expected bundles, create tokens only with write approval, and call GET endpoints through call_published_api before any non-GET checks."
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown prompt: {other}"),
                    None,
                ));
            }
        };
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            text,
        )]))
    }
}

pub async fn serve(args: McpArgs) -> anyhow::Result<()> {
    if args.transport.eq_ignore_ascii_case("stdio") {
        let client = DbapiClient::new(args.base_url)?;
        DbapiMcpServer::new(client, args.allow_write)
            .serve(rmcp::transport::stdio())
            .await?
            .waiting()
            .await?;
        return Ok(());
    }

    if !args.transport.eq_ignore_ascii_case("http") {
        anyhow::bail!(
            "unsupported MCP transport {}; supported transports are http and stdio",
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
    report.error("write-capable MCP tools require process --allow-write and tool allowWrite=true");
    Some(report)
}

fn structured(value: impl Serialize) -> Result<CallToolResult, McpError> {
    let value = serde_json::to_value(value).map_err(mcp_internal)?;
    Ok(CallToolResult::structured(value))
}

fn resource(uri: &str, name: &str, description: &str, mime_type: &str) -> rmcp::model::Resource {
    RawResource::new(uri, name)
        .with_description(description)
        .with_mime_type(mime_type)
        .no_annotation()
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
