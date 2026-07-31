use crate::{
    config, handler::handle_ipc_request, ipc_server, radius::RadiusPresence,
    supervisor::OrchestratorSupervisor,
};
use std::{
    io,
    sync::{Arc, Mutex},
};

pub async fn serve() -> io::Result<()> {
    std::fs::create_dir_all(config::orchestrator_dir())?;
    let supervisor = Arc::new(Mutex::new(OrchestratorSupervisor::new()));
    supervisor
        .lock()
        .map_err(|_| io::Error::other("supervisor lock poisoned"))?
        .recover_after_restart()?;
    let mut radius = RadiusPresence::default();
    let _ = radius.start(None)?;
    ipc_server::run_ipc_server(move |request| {
        let supervisor = supervisor.clone();
        async move {
            let mut supervisor = supervisor.lock().expect("supervisor lock poisoned");
            handle_ipc_request(&mut supervisor, request)
        }
    })
    .await
}
