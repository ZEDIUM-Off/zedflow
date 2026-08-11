use crate::{
    config,
    handler::{handle_ipc_request, open_rpc_stream},
    ipc_server,
    supervisor::OrchestratorSupervisor,
};
use std::{io, sync::Arc};

pub async fn serve() -> io::Result<()> {
    std::fs::create_dir_all(config::orchestrator_dir())?;
    let supervisor = Arc::new(tokio::sync::Mutex::new(OrchestratorSupervisor::new()));
    supervisor.lock().await.recover_after_restart().await?;
    supervisor.lock().await.start_radius().await?;
    let request_supervisor = supervisor.clone();
    ipc_server::run_ipc_server(
        move |request| {
            let supervisor = request_supervisor.clone();
            async move {
                let mut supervisor = supervisor.lock().await;
                handle_ipc_request(&mut supervisor, request).await
            }
        },
        move |instance_id, outgoing| {
            let supervisor = supervisor.clone();
            async move {
                let supervisor = supervisor.lock().await;
                open_rpc_stream(&*supervisor, &instance_id, outgoing)
            }
        },
    )
    .await
}
