use crate::{config, handler::handle_ipc_request, ipc_server, supervisor::OrchestratorSupervisor};
use std::{io, sync::Arc};

pub async fn serve() -> io::Result<()> {
    std::fs::create_dir_all(config::orchestrator_dir())?;
    let supervisor = Arc::new(tokio::sync::Mutex::new(OrchestratorSupervisor::new()));
    supervisor.lock().await.recover_after_restart().await?;
    supervisor.lock().await.start_radius().await?;
    ipc_server::run_ipc_server(move |request| {
        let supervisor = supervisor.clone();
        async move {
            let mut supervisor = supervisor.lock().await;
            handle_ipc_request(&mut supervisor, request).await
        }
    })
    .await
}
