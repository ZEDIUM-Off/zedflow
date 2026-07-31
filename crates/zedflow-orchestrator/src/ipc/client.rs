use crate::{
    config,
    ipc_protocol::{
        OrchestratorRequest, OrchestratorResponse, encode_message, parse_response_line,
    },
};
use std::io;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub async fn send_ipc_request(request: &OrchestratorRequest) -> io::Result<OrchestratorResponse> {
    let mut stream = UnixStream::connect(config::socket_path()).await?;
    stream
        .write_all(
            encode_message(request)
                .map_err(io::Error::other)?
                .as_bytes(),
        )
        .await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    if line.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Orchestrator socket closed before a response was received",
        ));
    }
    parse_response_line(line.trim()).map_err(io::Error::other)
}
