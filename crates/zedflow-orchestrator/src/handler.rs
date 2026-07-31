use crate::{
    ipc_protocol::{InstanceSummary, OrchestratorRequest, OrchestratorResponse},
    supervisor::OrchestratorSupervisor,
};

pub fn handle_ipc_request(
    supervisor: &mut OrchestratorSupervisor,
    request: OrchestratorRequest,
) -> OrchestratorResponse {
    match request {
        OrchestratorRequest::Spawn { cwd, label, .. } => {
            match supervisor.spawn_instance(cwd, label) {
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
        OrchestratorRequest::Stop { instance_id } => match supervisor.stop_instance(&instance_id) {
            Ok(Some(_)) => OrchestratorResponse::StopResult {
                ok: true,
                instance_id: Some(instance_id),
            },
            Ok(None) => unknown(&instance_id),
            Err(error) => error_response(error),
        },
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
