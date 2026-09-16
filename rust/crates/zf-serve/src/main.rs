use clap::Parser;
#[derive(Parser)]
struct Args {
    /// Print the immutable identity of this binary without opening data.
    #[arg(long)]
    build_info: bool,
    #[arg(long, default_value = "127.0.0.1:3142")]
    listen: std::net::SocketAddr,
    #[arg(long, default_value = ".zedflow")]
    data: std::path::PathBuf,
    #[arg(long, default_value = ".")]
    workspace: std::path::PathBuf,
    #[arg(long, default_value = "web/apps/client/dist")]
    web: std::path::PathBuf,
    #[arg(long = "skill-dir")]
    skill_dirs: Vec<std::path::PathBuf>,
    /// Override the home containing global .zedflow/.agents flows (isolated instances/tests).
    #[arg(long)]
    flow_home: Option<std::path::PathBuf>,
    /// Override only global instruction/skill sources; credentials keep their normal home.
    #[arg(long)]
    context_home: Option<std::path::PathBuf>,
    /// Offline, backed-up migration retaining this session and sessions newer than cutoff.
    #[arg(long, requires = "maintenance_cutoff")]
    maintain_keep_session: Option<String>,
    #[arg(long, requires = "maintain_keep_session")]
    maintenance_cutoff: Option<u64>,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.build_info {
        println!("{}", zf_serve::app_updates::build());
        return Ok(());
    }
    if let Some(id) = &args.maintain_keep_session {
        let report = zf_storage::migration::maintain(
            &args.data,
            id,
            args.maintenance_cutoff.expect("required cutoff"),
            &zf_runtime::archive_validation::AdkCheckpointCodec,
        )
        .await?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    zf_serve::server::serve(zf_serve::server::ServerOptions {
        listen: args.listen,
        data: args.data,
        workspace: args.workspace,
        web: args.web,
        skill_dirs: args.skill_dirs,
        flow_home: args.flow_home,
        context_home: args.context_home,
    })
    .await
}
