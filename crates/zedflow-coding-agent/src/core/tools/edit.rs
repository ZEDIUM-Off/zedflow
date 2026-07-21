use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, PrepareArgumentsFn, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use super::edit_diff::{
    Edit, apply_edits_to_normalized_content, detect_line_ending, error_code, generate_diff_string,
    generate_unified_patch, normalize_to_lf, restore_line_endings, strip_bom,
};
use super::file_mutation_queue::with_file_mutation_queue;
use super::path_utils::resolve_to_cwd;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditToolInput {
    pub path: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditToolDetails {
    pub diff: String,
    pub patch: String,
    pub first_changed_line: Option<usize>,
}

pub type EditToolResult = AgentToolResult<EditToolDetails>;
pub type EditOperationFuture<T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send>>;
pub type EditAccessOperation = Arc<dyn Fn(PathBuf) -> EditOperationFuture<()> + Send + Sync>;
pub type EditReadOperation = Arc<dyn Fn(PathBuf) -> EditOperationFuture<Vec<u8>> + Send + Sync>;
pub type EditWriteOperation = Arc<dyn Fn(PathBuf, String) -> EditOperationFuture<()> + Send + Sync>;

#[derive(Clone)]
pub struct EditOperations {
    pub access: EditAccessOperation,
    pub read_file: EditReadOperation,
    pub write_file: EditWriteOperation,
}

impl Default for EditOperations {
    fn default() -> Self {
        Self {
            access: Arc::new(|path| {
                Box::pin(async move {
                    tokio::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(path)
                        .await
                        .map(|_| ())
                        .map_err(|error| io::Error::new(error.kind(), error_code(&error)))
                })
            }),
            read_file: Arc::new(|path| Box::pin(tokio::fs::read(path))),
            write_file: Arc::new(|path, content| {
                Box::pin(async move { tokio::fs::write(path, content).await })
            }),
        }
    }
}

impl fmt::Debug for EditOperations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EditOperations")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct EditTool {
    cwd: PathBuf,
    operations: EditOperations,
}

impl EditTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self::with_operations(cwd, EditOperations::default())
    }

    pub fn with_operations(cwd: impl AsRef<Path>, operations: EditOperations) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
            operations,
        }
    }

    pub async fn execute(&self, input: EditToolInput) -> io::Result<EditToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: EditToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<EditToolResult> {
        if input.edits.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Edit tool input is invalid. edits must contain at least one replacement.",
            ));
        }
        let absolute_path = resolve_to_cwd(&input.path, &self.cwd)?;
        let path_display = input.path.clone();
        let edits = input.edits;
        let edit_count = edits.len();
        let queue_path = absolute_path.clone();
        let operations = self.operations.clone();

        with_file_mutation_queue(&absolute_path, || async move {
            check_aborted(signal)?;
            (operations.access)(queue_path.clone())
                .await
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Could not edit file: {path_display}. {error}."),
                    )
                })?;
            check_aborted(signal)?;

            let bytes = (operations.read_file)(queue_path.clone()).await?;
            check_aborted(signal)?;
            let raw = String::from_utf8_lossy(&bytes);
            let (bom, content) = strip_bom(&raw);
            let ending = detect_line_ending(content);
            let normalized = normalize_to_lf(content);
            let applied = apply_edits_to_normalized_content(&normalized, &edits, &path_display)?;
            check_aborted(signal)?;

            let final_content = format!(
                "{bom}{}",
                restore_line_endings(&applied.new_content, ending)
            );
            (operations.write_file)(queue_path, final_content).await?;
            check_aborted(signal)?;

            let diff = generate_diff_string(&applied.base_content, &applied.new_content, 4);
            let patch = generate_unified_patch(
                &path_display,
                &applied.base_content,
                &applied.new_content,
                4,
            );
            Ok(AgentToolResult {
                content: vec![text(format!(
                    "Successfully replaced {edit_count} block(s) in {path_display}."
                ))],
                details: EditToolDetails {
                    diff: diff.diff,
                    patch,
                    first_changed_line: diff.first_changed_line,
                },
                terminate: None,
            })
        })
        .await?
    }
}

pub fn prepare_edit_arguments(mut input: ToolSchema) -> Result<ToolSchema, AgentCallbackError> {
    let Some(args) = input.as_object_mut() else {
        return Ok(input);
    };

    if let Some(stringified) = args.get("edits").and_then(ToolSchema::as_str)
        && let Ok(parsed) = serde_yaml::from_str::<ToolSchema>(stringified)
        && parsed.is_array()
    {
        args.insert("edits".into(), parsed);
    }

    let Some(old_text) = args
        .get("oldText")
        .and_then(ToolSchema::as_str)
        .map(str::to_owned)
    else {
        return Ok(input);
    };
    let Some(new_text) = args
        .get("newText")
        .and_then(ToolSchema::as_str)
        .map(str::to_owned)
    else {
        return Ok(input);
    };

    let mut edits = args
        .get("edits")
        .and_then(ToolSchema::as_array)
        .cloned()
        .unwrap_or_default();
    let mut replacement = ToolSchema::Object(Default::default());
    replacement["oldText"] = old_text.into();
    replacement["newText"] = new_text.into();
    edits.push(replacement);
    args.remove("oldText");
    args.remove("newText");
    args.insert("edits".into(), ToolSchema::Array(edits));
    Ok(input)
}

pub fn create_edit_tool(cwd: impl AsRef<Path>) -> AgentTool {
    create_edit_tool_with_operations(cwd, EditOperations::default())
}

pub fn create_edit_tool_with_operations(
    cwd: impl AsRef<Path>,
    operations: EditOperations,
) -> AgentTool {
    let tool = EditTool::with_operations(cwd, operations);
    let prepare: PrepareArgumentsFn = Arc::new(prepare_edit_arguments);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = parse_input(&args)?;
            let result = tool
                .execute_with_signal(input, signal.as_ref())
                .await
                .map_err(|error| Box::new(error) as AgentCallbackError)?;
            let mut details = ToolSchema::Object(Default::default());
            details["diff"] = result.details.diff.into();
            details["patch"] = result.details.patch.into();
            if let Some(line) = result.details.first_changed_line {
                details["firstChangedLine"] = line.into();
            }
            Ok(AgentToolResult {
                content: result.content,
                details,
                terminate: result.terminate,
            })
        }) as AgentFuture<'_, _>
    });

    AgentTool {
        tool: Tool {
            name: "edit".into(),
            description: "Edit a single file using exact text replacement. Every edits[].oldText must match a unique, non-overlapping region of the original file. If two changes affect the same block or nearby lines, merge them into one edit instead of emitting overlapping edits. Do not include large unchanged regions just to connect distant changes.".into(),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"path":{"type":"string","description":"Path to the file to edit (relative or absolute)"},"edits":{"type":"array","description":"One or more targeted replacements. Each edit is matched against the original file, not incrementally. Do not include overlapping or nested edits.","items":{"type":"object","properties":{"oldText":{"type":"string"},"newText":{"type":"string"}},"required":["oldText","newText"]}}},"required":["path","edits"]}"#,
            )
            .expect("valid edit schema"),
        },
        label: "edit".into(),
        prepare_arguments: Some(prepare),
        execute,
        execution_mode: None,
    }
}

fn parse_input(args: &ToolSchema) -> Result<EditToolInput, AgentCallbackError> {
    let path = args
        .get("path")
        .and_then(ToolSchema::as_str)
        .unwrap_or_default()
        .to_owned();
    let edits = args
        .get("edits")
        .and_then(ToolSchema::as_array)
        .ok_or_else(|| {
            callback_error(
                "Edit tool input is invalid. edits must contain at least one replacement.",
            )
        })?
        .iter()
        .map(|edit| Edit {
            old_text: edit
                .get("oldText")
                .and_then(ToolSchema::as_str)
                .unwrap_or_default()
                .to_owned(),
            new_text: edit
                .get("newText")
                .and_then(ToolSchema::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return Err(callback_error(
            "Edit tool input is invalid. edits must contain at least one replacement.",
        ));
    }
    Ok(EditToolInput { path, edits })
}

fn callback_error(message: &str) -> AgentCallbackError {
    Box::new(io::Error::new(io::ErrorKind::InvalidInput, message))
}

fn check_aborted(signal: Option<&AbortSignal>) -> io::Result<()> {
    if signal.is_some_and(AbortSignal::aborted) {
        Err(io::Error::other("Operation aborted"))
    } else {
        Ok(())
    }
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}
