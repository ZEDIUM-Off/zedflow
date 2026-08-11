use crate::ipc_protocol::OrchestratorRequest;

pub fn parse_request(args: &[String], cwd: String) -> Result<Option<OrchestratorRequest>, String> {
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h" | "--version" | "-v" | "serve") => Ok(None),
        Some("list") => Ok(Some(OrchestratorRequest::List)),
        Some("spawn") => Ok(Some(OrchestratorRequest::Spawn {
            cwd: flag(args, "--cwd").unwrap_or(cwd),
            label: flag(args, "--label"),
            provider: None,
            model: None,
        })),
        Some("status") => {
            id(args).map(|instance_id| Some(OrchestratorRequest::Status { instance_id }))
        }
        Some("stop") => id(args).map(|instance_id| Some(OrchestratorRequest::Stop { instance_id })),
        Some("rpc") => {
            let instance_id = id(args)?;
            let command = args
                .get(2)
                .ok_or_else(|| "Usage: orchestrator rpc <instance-id> <json-command>".to_string())
                .and_then(|json| serde_json::from_str(json).map_err(|error| error.to_string()))?;
            Ok(Some(OrchestratorRequest::Rpc {
                instance_id,
                command,
            }))
        }
        Some("rpc-stream") => {
            id(args).map(|instance_id| Some(OrchestratorRequest::RpcStream { instance_id }))
        }
        Some(command) => Err(format!("Unknown command: {command}")),
    }
}
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}
fn id(args: &[String]) -> Result<String, String> {
    args.get(1)
        .cloned()
        .ok_or_else(|| "instance id is required".into())
}
