use crate::{
    config,
    ipc_protocol::{OrchestratorRequest, OrchestratorResponse, encode_message, parse_request_line},
};
use serde_json::Value;
use std::{future::Future, io, path::Path, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc,
};

pub struct RpcStream {
    handle_request: Arc<dyn Fn(Value) -> io::Result<()> + Send + Sync>,
    close: Option<Box<dyn FnOnce() + Send>>,
}

impl RpcStream {
    pub fn new(
        handle_request: impl Fn(Value) -> io::Result<()> + Send + Sync + 'static,
        close: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            handle_request: Arc::new(handle_request),
            close: Some(Box::new(close)),
        }
    }

    fn request_handler(&self) -> Arc<dyn Fn(Value) -> io::Result<()> + Send + Sync> {
        self.handle_request.clone()
    }
}

impl Drop for RpcStream {
    fn drop(&mut self) {
        if let Some(close) = self.close.take() {
            close();
        }
    }
}

pub async fn start_ipc_server() -> io::Result<UnixListener> {
    let path = config::socket_path();
    remove_stale_socket_if_needed(&path).await?;
    UnixListener::bind(path)
}

pub async fn run_ipc_server<H, F, O, G>(handler: H, open_rpc_stream: O) -> io::Result<()>
where
    H: Fn(OrchestratorRequest) -> F + Send + Sync + 'static,
    F: Future<Output = OrchestratorResponse> + Send + 'static,
    O: Fn(String, mpsc::UnboundedSender<Value>) -> G + Send + Sync + 'static,
    G: Future<Output = Option<RpcStream>> + Send + 'static,
{
    let listener = start_ipc_server().await?;
    let handler = Arc::new(handler);
    let open_rpc_stream = Arc::new(open_rpc_stream);
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle(stream, handler.clone(), open_rpc_stream.clone()));
    }
}

async fn handle<H, F, O, G>(stream: UnixStream, handler: Arc<H>, open_rpc_stream: Arc<O>)
where
    H: Fn(OrchestratorRequest) -> F + Send + Sync + 'static,
    F: Future<Output = OrchestratorResponse> + Send + 'static,
    O: Fn(String, mpsc::UnboundedSender<Value>) -> G + Send + Sync + 'static,
    G: Future<Output = Option<RpcStream>> + Send + 'static,
{
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let request = match parse_request_line(line.trim()) {
        Ok(request) => request,
        Err(error) => {
            write_and_close(
                reader.into_inner(),
                OrchestratorResponse::Error {
                    ok: false,
                    error: error.to_string(),
                },
            )
            .await;
            return;
        }
    };

    let OrchestratorRequest::RpcStream { instance_id } = request else {
        write_and_close(reader.into_inner(), handler(request).await).await;
        return;
    };
    let ready = handler(OrchestratorRequest::RpcStream {
        instance_id: instance_id.clone(),
    })
    .await;
    if !matches!(
        &ready,
        OrchestratorResponse::RpcReady {
            ok: true,
            instance: Some(_)
        }
    ) {
        write_and_close(reader.into_inner(), ready).await;
        return;
    }

    let (outgoing, mut incoming) = mpsc::unbounded_channel();
    let Some(rpc_stream) = open_rpc_stream(instance_id.clone(), outgoing.clone()).await else {
        write_and_close(
            reader.into_inner(),
            OrchestratorResponse::Error {
                ok: false,
                error: format!("Unknown instance: {instance_id}"),
            },
        )
        .await;
        return;
    };
    let _ = outgoing.send(serde_json::to_value(ready).unwrap_or_else(
        |error| serde_json::json!({"type":"error","ok":false,"error":error.to_string()}),
    ));

    let stream = reader.into_inner();
    let (read_half, mut write_half) = stream.into_split();
    let writer = tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            let Ok(message) = encode_message(&message) else {
                continue;
            };
            if write_half.write_all(message.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    let (requests, mut queued_requests) = mpsc::unbounded_channel::<String>();
    let handle_request = rpc_stream.request_handler();
    let request_outgoing = outgoing.clone();
    let request_worker = tokio::spawn(async move {
        while let Some(line) = queued_requests.recv().await {
            let request = match serde_json::from_str(&line) {
                Ok(request) => request,
                Err(error) => {
                    let _ = request_outgoing.send(
                        serde_json::json!({"type":"error","ok":false,"error":error.to_string()}),
                    );
                    continue;
                }
            };
            let handle_request = handle_request.clone();
            let outgoing = request_outgoing.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Err(error) = handle_request(request) {
                    let _ = outgoing.send(
                        serde_json::json!({"type":"error","ok":false,"error":error.to_string()}),
                    );
                }
            })
            .await;
        }
    });
    let mut lines = BufReader::new(read_half).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if !line.is_empty() && requests.send(line.to_owned()).is_err() {
            break;
        }
    }
    drop(requests);
    let _ = request_worker.await;
    drop(rpc_stream);
    drop(outgoing);
    let _ = writer.await;
}

async fn write_and_close(mut stream: UnixStream, response: OrchestratorResponse) {
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
