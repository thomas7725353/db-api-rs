use crate::dbapi_client::{DbapiClient, PublishedApiCall};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "db-api-rs")]
#[command(about = "DBAPI runtime, bundle generator, and MCP sidecar")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
    Bundle(BundleArgs),
    Mcp(McpArgs),
    Qa(QaArgs),
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: Option<McpCommand>,
    #[arg(long, default_value = "http")]
    pub transport: String,
    #[arg(long, default_value = "0.0.0.0:8521")]
    pub listen: String,
    #[arg(long, default_value = "http://127.0.0.1:8520")]
    pub base_url: String,
    #[arg(long, default_value_t = false)]
    pub allow_write: bool,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    Inspect(McpInspectArgs),
    Call(McpCallArgs),
}

#[derive(Debug, Args)]
pub struct McpInspectArgs {
    #[arg(long, default_value_t = true)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct McpCallArgs {
    pub tool: String,
    #[arg(long, default_value = "{}")]
    pub args_json: String,
}

#[derive(Debug, Args)]
pub struct QaArgs {
    #[command(subcommand)]
    pub command: QaCommand,
}

#[derive(Debug, Subcommand)]
pub enum QaCommand {
    Smoke(QaSmokeArgs),
}

#[derive(Debug, Args)]
pub struct QaSmokeArgs {
    #[arg(long, default_value = "http://127.0.0.1:8520")]
    pub base_url: String,
    #[arg(long)]
    pub path: Option<String>,
    #[arg(long, default_value = "GET")]
    pub method: String,
    #[arg(long, default_value = "{}")]
    pub params_json: String,
    #[arg(long)]
    pub authorization: Option<String>,
    #[arg(long, default_value_t = false)]
    pub allow_write: bool,
}

#[derive(Debug, Args)]
pub struct BundleArgs {
    #[command(subcommand)]
    pub command: BundleCommand,
}

#[derive(Debug, Subcommand)]
pub enum BundleCommand {
    DraftTable(DraftTableArgs),
    DraftSql(DraftSqlArgs),
    Validate(BundleIoArgs),
    Apply(BundleApplyArgs),
}

#[derive(Debug, Args)]
pub struct DraftTableArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub datasource_id: String,
    #[arg(long)]
    pub table: String,
    #[arg(long)]
    pub primary_key: Option<String>,
    #[arg(long)]
    pub resource_path: String,
    #[arg(long)]
    pub group_id: String,
    #[arg(long)]
    pub group_name: String,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct DraftSqlArgs {
    #[arg(long)]
    pub datasource_id: String,
    #[arg(long)]
    pub resource_path: String,
    #[arg(long)]
    pub api_id: String,
    #[arg(long)]
    pub api_name: String,
    #[arg(long)]
    pub group_id: String,
    #[arg(long)]
    pub group_name: String,
    #[arg(long)]
    pub sql: String,
    #[arg(long, default_value = "sql")]
    pub engine: String,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Debug, Args)]
pub struct BundleIoArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct BundleApplyArgs {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub dir: PathBuf,
    #[arg(long, default_value_t = false)]
    pub allow_write: bool,
}

pub async fn run() -> anyhow::Result<()> {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => crate::app::serve_http().await,
        Command::Bundle(args) => crate::bundle_files::run_bundle_command(args).await,
        Command::Mcp(args) => match &args.command {
            Some(McpCommand::Inspect(_)) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&crate::mcp_server::inspection_json())?
                );
                Ok(())
            }
            Some(McpCommand::Call(call_args)) => {
                let value: serde_json::Value = serde_json::from_str(&call_args.args_json)?;
                let response = crate::mcp_server::call_local_tool(
                    args.base_url.clone(),
                    args.allow_write,
                    &call_args.tool,
                    value,
                )
                .await?;
                println!("{}", serde_json::to_string_pretty(&response)?);
                Ok(())
            }
            None => crate::mcp_server::serve(args).await,
        },
        Command::Qa(args) => run_qa_command(args).await,
    }
}

async fn run_qa_command(args: QaArgs) -> anyhow::Result<()> {
    match args.command {
        QaCommand::Smoke(args) => {
            let client = DbapiClient::new(args.base_url)?;
            let health = client.health_check().await?;
            let mut result = serde_json::json!({
                "health": health
            });
            if let Some(path) = args.path {
                if !args.method.eq_ignore_ascii_case("GET") && !args.allow_write {
                    anyhow::bail!("non-GET smoke calls require --allow-write");
                }
                let params = serde_json::from_str(&args.params_json)?;
                let response = client
                    .call_published_api(PublishedApiCall {
                        method: args.method,
                        path,
                        params,
                        authorization: args.authorization,
                        headers: Default::default(),
                    })
                    .await?;
                result["api"] = serde_json::to_value(response)?;
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
    }
}
