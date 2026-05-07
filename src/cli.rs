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
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "http")]
    pub transport: String,
    #[arg(long, default_value = "0.0.0.0:8521")]
    pub listen: String,
    #[arg(long, default_value = "http://127.0.0.1:8520")]
    pub base_url: String,
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
        Command::Mcp(args) => crate::mcp_server::serve(args).await,
    }
}
