use crate::cli::{BundleArgs, BundleCommand};

pub async fn run_bundle_command(args: BundleArgs) -> anyhow::Result<()> {
    match args.command {
        BundleCommand::DraftTable(_) => anyhow::bail!("draft-table is not implemented yet"),
        BundleCommand::DraftSql(_) => anyhow::bail!("draft-sql is not implemented yet"),
        BundleCommand::Validate(_) => anyhow::bail!("validate is not implemented yet"),
        BundleCommand::Apply(_) => anyhow::bail!("apply is not implemented yet"),
    }
}
