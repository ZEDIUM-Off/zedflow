//! Lightweight RPC framing client for embedders.

use super::{
    jsonl::{JsonlReader, serialize_json_line},
    rpc_types::{RpcCommand, RpcResponse},
};
use serde::Serialize;
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, Default)]
pub struct RpcClient {
    next_id: u64,
}

impl RpcClient {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn encode(&mut self, mut command: RpcCommand) -> String {
        self.next_id += 1;
        let id = format!("req_{}", self.next_id);
        match &mut command {
            RpcCommand::ExtensionUiResponse { id: current, .. } => *current = id,
            RpcCommand::Prompt { id: current, .. }
            | RpcCommand::Steer { id: current, .. }
            | RpcCommand::FollowUp { id: current, .. }
            | RpcCommand::Abort { id: current }
            | RpcCommand::NewSession { id: current, .. }
            | RpcCommand::GetState { id: current }
            | RpcCommand::SetModel { id: current, .. }
            | RpcCommand::CycleModel { id: current }
            | RpcCommand::GetAvailableModels { id: current }
            | RpcCommand::SetThinkingLevel { id: current, .. }
            | RpcCommand::CycleThinkingLevel { id: current }
            | RpcCommand::SetSteeringMode { id: current, .. }
            | RpcCommand::SetFollowUpMode { id: current, .. }
            | RpcCommand::Compact { id: current, .. }
            | RpcCommand::SetAutoCompaction { id: current, .. }
            | RpcCommand::SetAutoRetry { id: current, .. }
            | RpcCommand::AbortRetry { id: current }
            | RpcCommand::Bash { id: current, .. }
            | RpcCommand::AbortBash { id: current }
            | RpcCommand::GetSessionStats { id: current }
            | RpcCommand::ExportHtml { id: current, .. }
            | RpcCommand::SwitchSession { id: current, .. }
            | RpcCommand::Fork { id: current, .. }
            | RpcCommand::Clone { id: current }
            | RpcCommand::GetForkMessages { id: current }
            | RpcCommand::GetEntries { id: current, .. }
            | RpcCommand::GetTree { id: current }
            | RpcCommand::GetLastAssistantText { id: current }
            | RpcCommand::SetSessionName { id: current, .. }
            | RpcCommand::GetMessages { id: current }
            | RpcCommand::GetCommands { id: current } => *current = Some(id),
        }
        serialize_json_line(&command)
    }

    pub fn decode(line: &str) -> serde_json::Result<RpcResponse> {
        serde_json::from_str(line)
    }
}

/// Copy JSONL records from reader to writer while validating framing.
pub fn relay_jsonl<R: BufRead, W: Write>(reader: R, mut writer: W) -> io::Result<usize> {
    let mut count = 0;
    for line in reader.lines() {
        let line = line?;
        let _: serde_json::Value = serde_json::from_str(&line).map_err(io::Error::other)?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        count += 1;
    }
    Ok(count)
}

#[allow(dead_code)]
fn _serialize<T: Serialize>(value: &T) -> String {
    serialize_json_line(value)
}

#[allow(dead_code)]
fn _reader() -> JsonlReader {
    JsonlReader::new()
}
