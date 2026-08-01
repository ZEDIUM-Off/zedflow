use crate::{
    ipc_protocol::{InstanceSummary, OrchestratorRequest, OrchestratorResponse},
    ipc_server::RpcStream,
    supervisor::OrchestratorSupervisor,
};
use serde_json::Value;
use tokio::sync::mpsc;

pub async fn handle_ipc_request(
    supervisor: &mut OrchestratorSupervisor,
    request: OrchestratorRequest,
) -> OrchestratorResponse {
    match request {
        OrchestratorRequest::Spawn { cwd, label, .. } => {
            match supervisor.spawn_instance(cwd, label).await {
                Ok(instance) => OrchestratorResponse::SpawnResult {
                    ok: true,
                    instance: Some(instance.into()),
                },
                Err(error) => error_response(error),
            }
        }
        OrchestratorRequest::List => match supervisor.list_instances() {
            Ok(instances) => OrchestratorResponse::ListResult {
                ok: true,
                instances: Some(instances.into_iter().map(InstanceSummary::from).collect()),
            },
            Err(error) => error_response(error),
        },
        OrchestratorRequest::Status { instance_id } => {
            match supervisor.get_instance(&instance_id) {
                Ok(Some(instance)) => OrchestratorResponse::StatusResult {
                    ok: true,
                    instance: Some(instance.into()),
                },
                Ok(None) => unknown(&instance_id),
                Err(error) => error_response(error),
            }
        }
        OrchestratorRequest::Stop { instance_id } => {
            match supervisor.stop_instance(&instance_id).await {
                Ok(Some(_)) => OrchestratorResponse::StopResult {
                    ok: true,
                    instance_id: Some(instance_id),
                },
                Ok(None) => unknown(&instance_id),
                Err(error) => error_response(error),
            }
        }
        OrchestratorRequest::Rpc {
            instance_id,
            command,
        } => match supervisor.handle_rpc(&instance_id, command) {
            Ok(Some(response)) => OrchestratorResponse::RpcResult { ok: true, response },
            Ok(None) => unknown(&instance_id),
            Err(error) => error_response(error),
        },
        OrchestratorRequest::RpcStream { instance_id } => {
            match supervisor.get_instance(&instance_id) {
                Ok(Some(instance)) => OrchestratorResponse::RpcReady {
                    ok: true,
                    instance: Some(instance.into()),
                },
                Ok(None) => unknown(&instance_id),
                Err(error) => error_response(error),
            }
        }
    }
}
pub fn open_rpc_stream(
    supervisor: &OrchestratorSupervisor,
    instance_id: &str,
    outgoing: mpsc::UnboundedSender<Value>,
) -> Option<RpcStream> {
    let handle = supervisor.open_rpc_stream(instance_id, outgoing.clone(), outgoing.clone())?;
    let request_handle = handle.clone();
    Some(RpcStream::new(
        move |request| {
            if request.get("type").and_then(Value::as_str) == Some("extension_ui_response") {
                return request_handle.handle_ui_response(&request);
            }
            let _ = outgoing.send(request_handle.handle_rpc(request)?);
            Ok(())
        },
        move || handle.close(),
    ))
}

fn unknown(id: &str) -> OrchestratorResponse {
    OrchestratorResponse::Error {
        ok: false,
        error: format!("Unknown instance: {id}"),
    }
}
fn error_response(error: impl std::fmt::Display) -> OrchestratorResponse {
    OrchestratorResponse::Error {
        ok: false,
        error: error.to_string(),
    }
}
