use crate::{
    config,
    ipc_protocol::{OrchestratorRequest, OrchestratorResponse, encode_message, parse_request_line},
};
use std::{future::Future, io, path::Path, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

pub async fn start_ipc_server() -> io::Result<UnixListener> {
    let path = config::socket_path();
    remove_stale_socket_if_needed(&path).await?;
    UnixListener::bind(path)
}

pub async fn run_ipc_server<H, F>(handler: H) -> io::Result<()>
where
    H: Fn(OrchestratorRequest) -> F + Send + Sync + 'static,
    F: Future<Output = OrchestratorResponse> + Send + 'static,
{
    let path = config::socket_path();
    remove_stale_socket_if_needed(&path).await?;
    let listener = UnixListener::bind(path)?;
    let handler = Arc::new(handler);
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle(stream, handler.clone()));
    }
}
async fn handle<H, F>(stream: UnixStream, handler: Arc<H>)
where
    H: Fn(OrchestratorRequest) -> F + Send + Sync + 'static,
    F: Future<Output = OrchestratorResponse> + Send + 'static,
{
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let response = match reader.read_line(&mut line).await {
        Ok(_) => match parse_request_line(line.trim()) {
            Ok(request) => handler(request).await,
            Err(error) => OrchestratorResponse::Error {
                ok: false,
                error: error.to_string(),
            },
        },
        Err(error) => OrchestratorResponse::Error {
            ok: false,
            error: error.to_string(),
        },
    };
    let mut stream = reader.into_inner();
    let _ = stream
        .write_all(
            encode_message(&response)
                .unwrap_or_else(|error| {
                    format!("{{\"type\":\"error\",\"ok\":false,\"error\":{error:?}}}\n")
                })
                .as_bytes(),
        )
        .await;
}
async fn remove_stale_socket_if_needed(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    match UnixStream::connect(path).await {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("orchestrator is already running: {}", path.display()),
        )),
        Err(_) => std::fs::remove_file(path),
    }
}
