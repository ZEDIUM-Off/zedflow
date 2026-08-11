use std::{env, process};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use zedflow_orchestrator::{
    cli::parse_request,
    config::{self, VERSION},
    ipc_client::send_ipc_request,
    ipc_protocol::{OrchestratorRequest, encode_message},
    serve::serve,
};

fn help() -> String {
    format!(
        "orchestrator v{VERSION}\n\nUsage:\n  orchestrator serve\n  orchestrator list\n  orchestrator spawn [--cwd <path>] [--label <label>]\n  orchestrator status <instance-id>\n  orchestrator stop <instance-id>\n  orchestrator rpc <instance-id> <json-command>\n  orchestrator rpc-stream <instance-id>\n  orchestrator --help\n  orchestrator --version\n\nRPC stream stdin expects JSONL RpcCommand or extension_ui_response messages."
    )
}

async fn rpc_stream(instance_id: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut socket = tokio::net::UnixStream::connect(config::socket_path()).await?;
    socket
        .write_all(
            encode_message(&OrchestratorRequest::RpcStream {
                instance_id: instance_id.clone(),
            })?
            .as_bytes(),
        )
        .await?;
    eprintln!(
        "connected to rpc stream {instance_id}; send JSONL RpcCommand or extension_ui_response on stdin"
    );

    let (reader, mut writer) = socket.into_split();
    let incoming = async move {
        let mut reader = BufReader::new(reader);
        tokio::io::copy(&mut reader, &mut tokio::io::stdout()).await?;
        Ok::<_, std::io::Error>(())
    };
    let outgoing = async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let message: serde_json::Value =
                serde_json::from_str(&line).map_err(std::io::Error::other)?;
            writer
                .write_all(
                    encode_message(&message)
                        .map_err(std::io::Error::other)?
                        .as_bytes(),
                )
                .await?;
        }
        std::future::pending::<std::io::Result<()>>().await
    };
    tokio::select! {
        result = incoming => result?,
        result = outgoing => result?,
    }
    Ok(())
}

fn usage(command: &str) -> Option<&'static str> {
    match command {
        "status" => Some("Usage: orchestrator status <instance-id>"),
        "stop" => Some("Usage: orchestrator stop <instance-id>"),
        "rpc" => Some("Usage: orchestrator rpc <instance-id> <json-command>"),
        "rpc-stream" => Some("Usage: orchestrator rpc-stream <instance-id>"),
        _ => None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h") => {
            println!("{}", help());
            return;
        }
        Some("--version" | "-v") => {
            println!("{VERSION}");
            return;
        }
        Some("serve") => {
            if let Err(error) = serve().await {
                eprintln!("{error}");
                process::exit(1);
            }
            return;
        }
        _ => {}
    }

    let request = match parse_request(
        &args,
        env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    ) {
        Ok(Some(request)) => request,
        Ok(None) => return,
        Err(error) => {
            if error.starts_with("Unknown command:") {
                eprintln!("{error}");
                println!("{}", help());
            } else {
                eprintln!("{}", usage(&args[0]).unwrap_or(&error));
            }
            process::exit(1);
        }
    };

    let result = match request {
        OrchestratorRequest::RpcStream { instance_id } => rpc_stream(instance_id).await,
        request => send_ipc_request(&request)
            .await
            .map_err(|error| error.into())
            .and_then(|response| {
                println!("{}", serde_json::to_string_pretty(&response)?);
                Ok(())
            }),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_the_runtime_commands() {
        let help = help();
        for command in [
            "serve",
            "list",
            "spawn",
            "status",
            "stop",
            "rpc",
            "rpc-stream",
        ] {
            assert!(help.contains(&format!("orchestrator {command}")));
        }
    }
}
