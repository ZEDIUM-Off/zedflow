//! Explicit test lifecycle: HTTP router and its durable execution owner.
use std::{path::PathBuf, sync::Arc};
use zf_execution::service::{ExecutionOptions, ExecutionService};

pub async fn open_router(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    home: PathBuf,
) -> anyhow::Result<(axum::Router, ExecutionService)> {
    std::fs::create_dir_all(&home)?;
    let service = ExecutionService::open(ExecutionOptions {
        data: data.clone(),
        workspace: workspace.clone(),
        flow_home: home.clone(),
        context_home: Some(home.clone()),
        skill_dirs: skill_dirs.clone(),
        authorizer: Arc::new(zf_serve::server::LocalAuthorizer),
    })
    .await?;
    let router = zf_serve::server::router_for_service(
        service.clone(),
        data,
        workspace,
        skill_dirs,
        home.clone(),
        Some(home),
        None,
        tokio_util::sync::CancellationToken::new(),
        Arc::new(zf_serve::app_updates::Updates::new(
            zf_serve::app_updates::Config {
                web: None,
                root: None,
                release: None,
            },
        )),
    )
    .await?;
    Ok((router, service))
}
