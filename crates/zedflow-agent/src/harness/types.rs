//! Harness contracts ported from Pi `packages/agent/src/harness/types.ts`.
//!
//! This module defines data, error, filesystem, shell, session, compaction, and
//! harness event contracts. Concrete storage, environment, compaction, and
//! harness behavior is intentionally owned by later port units.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use zedflow_ai::utils::validation::ToolValidationError;
use zedflow_ai::{CacheRetention, ImageContent, Model, Models, SimpleStreamOptions, Transport};

use crate::types::{
    AgentEvent, AgentMessage, AgentTool, AgentToolResultContent, QueueMode, ThinkingLevel,
};

/// Result of a fallible harness operation.
pub type Result<TValue, TError> = std::result::Result<TValue, TError>;

/// Boxed future used by harness trait contracts.
pub type HarnessFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Create a successful [`Result`].
#[must_use]
pub fn ok<TValue, TError>(value: TValue) -> Result<TValue, TError> {
    Ok(value)
}

/// Create a failed [`Result`].
#[must_use]
pub fn err<TValue, TError>(error: TError) -> Result<TValue, TError> {
    Err(error)
}

/// Return the success value or panic with the failure error.
///
/// Intended only for tests and explicit adapter boundaries, mirroring Pi's
/// `getOrThrow()` helper.
///
/// # Panics
///
/// Panics when `result` is [`Err`].
pub fn get_or_throw<TValue, TError: fmt::Debug>(result: Result<TValue, TError>) -> TValue {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{error:?}"),
    }
}

/// Return the success value or `None`.
#[must_use]
pub fn get_or_none<TValue, TError>(result: Result<TValue, TError>) -> Option<TValue> {
    result.ok()
}

/// Normalize displayable error values into strings for typed error causes.
#[must_use]
pub fn to_error(error: impl fmt::Display) -> String {
    error.to_string()
}

/// Skill loaded from a `SKILL.md` file or provided by an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Stable skill name used for lookup and model-visible listings.
    pub name: String,
    /// Short model-visible description of when to use the skill.
    pub description: String,
    /// Full skill instructions.
    pub content: String,
    /// Absolute path to the skill file.
    pub file_path: String,
    /// Exclude this skill from model-visible skill lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_model_invocation: Option<bool>,
}

/// Prompt template that can be formatted into a prompt for explicit invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Stable template name used for lookup or application command routing.
    pub name: String,
    /// Optional description for command lists or autocomplete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Template content.
    pub content: String,
}

/// Resources made available to explicit invocation methods and system-prompt callbacks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHarnessResources<TSkill = Skill, TPromptTemplate = PromptTemplate> {
    /// Prompt templates available for explicit invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_templates: Option<Vec<TPromptTemplate>>,
    /// Skills available to the model and explicit skill invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<TSkill>>,
}

/// Curated provider request options owned by the harness and snapshotted per turn.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHarnessStreamOptions {
    /// Preferred transport forwarded to the stream function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    /// Provider request timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Maximum provider retry attempts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    /// Optional cap for provider-requested retry delays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    /// Additional request headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Provider metadata forwarded with requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
    /// Provider cache retention hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
}

impl From<AgentHarnessStreamOptions> for SimpleStreamOptions {
    fn from(value: AgentHarnessStreamOptions) -> Self {
        let mut options = SimpleStreamOptions::default();
        options.stream.transport = value.transport;
        options.stream.timeout_ms = value.timeout_ms;
        options.stream.max_retries = value.max_retries;
        options.stream.max_retry_delay_ms = value.max_retry_delay_ms;
        options.stream.headers = value.headers.map(|headers| {
            headers
                .into_iter()
                .map(|(key, value)| (key, Some(value)))
                .collect()
        });
        options.stream.metadata = value.metadata;
        options.stream.cache_retention = value.cache_retention;
        options
    }
}

/// Per-request stream option patch returned by provider hooks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHarnessStreamOptionsPatch {
    /// Preferred transport patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    /// Provider timeout patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    /// Maximum retry delay patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    /// Header patch; `None` values delete keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, Option<String>>>,
    /// Metadata patch; `None` values delete keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Option<Value>>>,
    /// Cache retention patch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
}

/// Kind of filesystem object addressed by a [`FileSystem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symbolic link.
    Symlink,
}

/// Stable file error codes returned by [`FileSystem`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileErrorCode {
    /// Operation was aborted.
    Aborted,
    /// Addressed path was not found.
    NotFound,
    /// Permission was denied.
    PermissionDenied,
    /// Expected a directory.
    NotDirectory,
    /// Expected a non-directory.
    IsDirectory,
    /// Invalid path or data.
    Invalid,
    /// Backend does not support the operation.
    NotSupported,
    /// Unknown backend failure.
    Unknown,
}

/// Error returned by filesystem operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileError {
    /// Backend-independent error code.
    pub code: FileErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Addressed path associated with the failure, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl FileError {
    /// Create a filesystem error.
    #[must_use]
    pub fn new(
        code: FileErrorCode,
        message: impl Into<String>,
        path: Option<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            path,
            cause,
        }
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for FileError {}

/// Stable execution error codes returned by [`Shell::exec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionErrorCode {
    /// Operation was aborted.
    Aborted,
    /// Command exceeded its timeout.
    Timeout,
    /// No shell is available.
    ShellUnavailable,
    /// Process spawn failed.
    SpawnError,
    /// Output callback failed.
    CallbackError,
    /// Unknown backend failure.
    Unknown,
}

/// Error returned by shell execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionError {
    /// Backend-independent error code.
    pub code: ExecutionErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl ExecutionError {
    /// Create an execution error.
    #[must_use]
    pub fn new(
        code: ExecutionErrorCode,
        message: impl Into<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for ExecutionError {}

/// Stable compaction error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionErrorCode {
    /// Operation was aborted.
    Aborted,
    /// Summarization failed.
    SummarizationFailed,
    /// Session state was invalid.
    InvalidSession,
    /// Unknown failure.
    Unknown,
}

/// Error returned by compaction helpers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionError {
    /// Stable error code.
    pub code: CompactionErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl CompactionError {
    /// Create a compaction error.
    #[must_use]
    pub fn new(
        code: CompactionErrorCode,
        message: impl Into<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for CompactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for CompactionError {}

/// Stable branch-summary error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchSummaryErrorCode {
    /// Operation was aborted.
    Aborted,
    /// Summarization failed.
    SummarizationFailed,
    /// Session state was invalid.
    InvalidSession,
}

/// Error returned by branch summarization helpers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchSummaryError {
    /// Stable error code.
    pub code: BranchSummaryErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl BranchSummaryError {
    /// Create a branch-summary error.
    #[must_use]
    pub fn new(
        code: BranchSummaryErrorCode,
        message: impl Into<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for BranchSummaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for BranchSummaryError {}

/// Stable session error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionErrorCode {
    /// Requested session or entry was not found.
    NotFound,
    /// Session data was invalid.
    InvalidSession,
    /// Session entry was invalid.
    InvalidEntry,
    /// Fork target was invalid.
    InvalidForkTarget,
    /// Storage backend failed.
    Storage,
    /// Unknown failure.
    Unknown,
}

/// Error returned by session storage, repositories, and tree operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionError {
    /// Stable error code.
    pub code: SessionErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl SessionError {
    /// Create a session error.
    #[must_use]
    pub fn new(code: SessionErrorCode, message: impl Into<String>, cause: Option<String>) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for SessionError {}

/// Stable top-level harness error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHarnessErrorCode {
    /// Harness is busy.
    Busy,
    /// Harness state does not allow the requested operation.
    InvalidState,
    /// Caller provided an invalid argument.
    InvalidArgument,
    /// Session subsystem failed.
    Session,
    /// Hook failed.
    Hook,
    /// Authentication failed.
    Auth,
    /// Compaction failed.
    Compaction,
    /// Branch summarization failed.
    BranchSummary,
    /// Unknown failure.
    Unknown,
}

/// Public AgentHarness failure with a stable top-level classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentHarnessError {
    /// Stable error code.
    pub code: AgentHarnessErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl AgentHarnessError {
    /// Create a harness error.
    #[must_use]
    pub fn new(
        code: AgentHarnessErrorCode,
        message: impl Into<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for AgentHarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for AgentHarnessError {}

/// Metadata for one filesystem object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// Basename of `path`.
    pub name: String,
    /// Absolute addressed path.
    pub path: String,
    /// Object kind.
    pub kind: FileKind,
    /// Size in bytes.
    pub size: u64,
    /// Modification time as milliseconds since Unix epoch.
    pub mtime_ms: u64,
}

/// Content passed to file write/append operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    /// UTF-8 text content.
    Text(String),
    /// Binary content.
    Binary(Vec<u8>),
}

/// Options for reading text lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadTextLinesOptions {
    /// Stop after this many lines.
    pub max_lines: Option<usize>,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
}

/// Options for directory creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDirOptions {
    /// Create missing parents.
    pub recursive: bool,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
}

impl Default for CreateDirOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            abort_signal: None,
        }
    }
}

/// Options for file or directory removal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoveOptions {
    /// Remove directories recursively.
    pub recursive: bool,
    /// Ignore missing paths when supported.
    pub force: bool,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
}

/// Options for temporary file creation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreateTempFileOptions {
    /// Filename prefix.
    pub prefix: Option<String>,
    /// Filename suffix.
    pub suffix: Option<String>,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
}

/// Filesystem capability used by the harness.
pub trait FileSystem: Send + Sync {
    /// Current working directory for relative paths.
    fn cwd(&self) -> &str;
    /// Return an absolute addressed path.
    fn absolute_path<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Join path segments in the filesystem namespace.
    fn join_path<'a>(
        &'a self,
        parts: &'a [String],
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Read a UTF-8 text file.
    fn read_text_file<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Read UTF-8 text lines.
    fn read_text_lines<'a>(
        &'a self,
        path: &'a str,
        options: ReadTextLinesOptions,
    ) -> HarnessFuture<'a, Result<Vec<String>, FileError>>;
    /// Read a binary file.
    fn read_binary_file<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<Vec<u8>, FileError>>;
    /// Create or overwrite a file.
    fn write_file<'a>(
        &'a self,
        path: &'a str,
        content: FileContent,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<(), FileError>>;
    /// Create or append to a file.
    fn append_file<'a>(
        &'a self,
        path: &'a str,
        content: FileContent,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<(), FileError>>;
    /// Return metadata for the addressed path.
    fn file_info<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<FileInfo, FileError>>;
    /// List direct children of a directory.
    fn list_dir<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<Vec<FileInfo>, FileError>>;
    /// Return canonical path for an existing path.
    fn canonical_path<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Return false for missing paths.
    fn exists<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<bool, FileError>>;
    /// Create a directory.
    fn create_dir<'a>(
        &'a self,
        path: &'a str,
        options: CreateDirOptions,
    ) -> HarnessFuture<'a, Result<(), FileError>>;
    /// Remove a file or directory.
    fn remove<'a>(
        &'a self,
        path: &'a str,
        options: RemoveOptions,
    ) -> HarnessFuture<'a, Result<(), FileError>>;
    /// Create a temporary directory and return its absolute path.
    fn create_temp_dir<'a>(
        &'a self,
        prefix: Option<&'a str>,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Create a temporary file and return its absolute path.
    fn create_temp_file<'a>(
        &'a self,
        options: CreateTempFileOptions,
    ) -> HarnessFuture<'a, Result<String, FileError>>;
    /// Release filesystem resources best-effort.
    fn cleanup<'a>(&'a self) -> HarnessFuture<'a, ()>;
}

/// Options for [`Shell::exec`].
#[derive(Clone, Default)]
pub struct ShellExecOptions {
    /// Working directory for the command.
    pub cwd: Option<String>,
    /// Additional environment variables.
    pub env: Option<HashMap<String, String>>,
    /// Timeout in seconds.
    pub timeout: Option<u64>,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
    /// Stdout chunk callback.
    pub on_stdout: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// Stderr chunk callback.
    pub on_stderr: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl fmt::Debug for ShellExecOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellExecOptions")
            .field("cwd", &self.cwd)
            .field("env", &self.env)
            .field("timeout", &self.timeout)
            .field("abort_signal", &self.abort_signal)
            .field("has_on_stdout", &self.on_stdout.is_some())
            .field("has_on_stderr", &self.on_stderr.is_some())
            .finish()
    }
}

/// Result of shell execution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellExecOutput {
    /// Collected stdout.
    pub stdout: String,
    /// Collected stderr.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
}

/// Shell execution capability used by the harness.
pub trait Shell: Send + Sync {
    /// Execute a shell command.
    fn exec<'a>(
        &'a self,
        command: &'a str,
        options: Option<ShellExecOptions>,
    ) -> HarnessFuture<'a, Result<ShellExecOutput, ExecutionError>>;
    /// Release shell resources best-effort.
    fn cleanup<'a>(&'a self) -> HarnessFuture<'a, ()>;
}

/// Filesystem and process execution environment used by the harness.
pub trait ExecutionEnv: FileSystem + Shell {}

impl<T> ExecutionEnv for T where T: FileSystem + Shell {}

/// Shared session tree entry fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTreeEntryBase {
    /// Entry id.
    pub id: String,
    /// Parent entry id, or null at root.
    pub parent_id: Option<String>,
    /// Entry timestamp as an ISO string.
    pub timestamp: String,
}

/// Stored message entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Agent message payload.
    pub message: AgentMessage,
}

/// Stored thinking-level change entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingLevelChangeEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// New thinking level.
    pub thinking_level: String,
}

/// Stored model change entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChangeEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Provider id.
    pub provider: String,
    /// Model id.
    pub model_id: String,
}

/// Stored active-tools change entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveToolsChangeEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Active tool names.
    pub active_tool_names: Vec<String>,
}

/// Stored compaction entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionEntry<T = Value> {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Summary text.
    pub summary: String,
    /// First entry kept after compaction.
    pub first_kept_entry_id: String,
    /// Token count before compaction.
    pub tokens_before: u64,
    /// Optional compaction details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<T>,
    /// Whether the entry came from a hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_hook: Option<bool>,
}

/// Stored branch-summary entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummaryEntry<T = Value> {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Branch source id.
    pub from_id: String,
    /// Summary text.
    pub summary: String,
    /// Optional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<T>,
    /// Whether the entry came from a hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_hook: Option<bool>,
}

/// Stored custom data entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEntry<T = Value> {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Application-defined entry type.
    pub custom_type: String,
    /// Application-defined data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

/// Stored custom message entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomMessageEntry<T = Value> {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Application-defined message type.
    pub custom_type: String,
    /// Message content.
    pub content: CustomMessageContent,
    /// Optional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<T>,
    /// Whether to display the custom message.
    pub display: bool,
}

/// Custom message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content.
    Blocks(Vec<AgentToolResultContent>),
}

/// Stored label entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Target entry id.
    pub target_id: String,
    /// Optional label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Stored session info entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfoEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Optional session name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Stored active leaf entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafEntry {
    /// Shared entry fields.
    #[serde(flatten)]
    pub base: SessionTreeEntryBase,
    /// Target leaf id.
    pub target_id: Option<String>,
}

/// Session tree entry union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionTreeEntry {
    /// Message entry.
    Message(MessageEntry),
    /// Thinking level change.
    ThinkingLevelChange(ThinkingLevelChangeEntry),
    /// Model change.
    ModelChange(ModelChangeEntry),
    /// Active tool names change.
    ActiveToolsChange(ActiveToolsChangeEntry),
    /// Compaction entry.
    Compaction(CompactionEntry),
    /// Branch summary entry.
    BranchSummary(BranchSummaryEntry),
    /// Custom data entry.
    Custom(CustomEntry),
    /// Custom message entry.
    CustomMessage(CustomMessageEntry),
    /// Label entry.
    Label(LabelEntry),
    /// Session info entry.
    SessionInfo(SessionInfoEntry),
    /// Leaf marker entry.
    Leaf(LeafEntry),
}

/// Reconstructed session context.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
    /// Messages visible from the current branch.
    pub messages: Vec<AgentMessage>,
    /// Current thinking level.
    pub thinking_level: String,
    /// Current model, if known.
    pub model: Option<SessionModelRef>,
    /// Current active tool names, if known.
    pub active_tool_names: Option<Vec<String>>,
}

/// Provider/model reference stored in session context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionModelRef {
    /// Provider id.
    pub provider: String,
    /// Model id.
    pub model_id: String,
}

/// Base session metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    /// Session id.
    pub id: String,
    /// Creation timestamp as an ISO string.
    pub created_at: String,
}

/// JSONL session metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonlSessionMetadata {
    /// Base metadata.
    #[serde(flatten)]
    pub base: SessionMetadata,
    /// Working directory associated with the session.
    pub cwd: String,
    /// JSONL file path.
    pub path: String,
    /// Optional parent session path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_path: Option<String>,
}

/// Session storage contract.
pub trait SessionStorage: Send + Sync {
    /// Return storage metadata.
    fn get_metadata<'a>(&'a self) -> HarnessFuture<'a, SessionMetadata>;
    /// Return current leaf id.
    fn get_leaf_id<'a>(&'a self) -> HarnessFuture<'a, Option<String>>;
    /// Persist the active leaf id.
    fn set_leaf_id<'a>(&'a self, leaf_id: Option<String>) -> HarnessFuture<'a, ()>;
    /// Create a new entry id.
    fn create_entry_id<'a>(&'a self) -> HarnessFuture<'a, String>;
    /// Append an entry.
    fn append_entry<'a>(
        &'a self,
        entry: SessionTreeEntry,
    ) -> HarnessFuture<'a, Result<(), SessionError>>;
    /// Return an entry by id.
    fn get_entry<'a>(&'a self, id: &'a str) -> HarnessFuture<'a, Option<SessionTreeEntry>>;
    /// Find entries by type name.
    fn find_entries<'a>(&'a self, entry_type: &'a str) -> HarnessFuture<'a, Vec<SessionTreeEntry>>;
    /// Return a label for target id.
    fn get_label<'a>(&'a self, id: &'a str) -> HarnessFuture<'a, Option<String>>;
    /// Return path from leaf to root.
    fn get_path_to_root<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> HarnessFuture<'a, Vec<SessionTreeEntry>>;
    /// Return all entries.
    fn get_entries<'a>(&'a self) -> HarnessFuture<'a, Vec<SessionTreeEntry>>;
}

/// Session behavior contract. Concrete implementation is owned by A2.
pub trait Session: Send + Sync {
    /// Return session metadata.
    fn get_metadata<'a>(&'a self) -> HarnessFuture<'a, SessionMetadata>;
    /// Return current leaf id.
    fn get_leaf_id<'a>(&'a self) -> HarnessFuture<'a, Option<String>>;
    /// Return an entry by id.
    fn get_entry<'a>(&'a self, id: &'a str) -> HarnessFuture<'a, Option<SessionTreeEntry>>;
    /// Return all entries.
    fn get_entries<'a>(&'a self) -> HarnessFuture<'a, Vec<SessionTreeEntry>>;
    /// Return branch entries from the specified id or current leaf.
    fn get_branch<'a>(
        &'a self,
        from_id: Option<String>,
    ) -> HarnessFuture<'a, Vec<SessionTreeEntry>>;
    /// Build current branch context.
    fn build_context<'a>(&'a self) -> HarnessFuture<'a, SessionContext>;
    /// Append an agent message.
    fn append_message<'a>(
        &'a self,
        message: AgentMessage,
    ) -> HarnessFuture<'a, Result<String, SessionError>>;
    /// Append a model selection entry.
    fn append_model_change<'a>(
        &'a self,
        provider: String,
        model_id: String,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("model changes are not supported for {provider}/{model_id}"),
                None,
            ))
        })
    }
    /// Append a thinking-level entry.
    fn append_thinking_level_change<'a>(
        &'a self,
        thinking_level: String,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("thinking-level changes are not supported for {thinking_level}"),
                None,
            ))
        })
    }
    /// Append an active-tools entry.
    fn append_active_tools_change<'a>(
        &'a self,
        active_tool_names: Vec<String>,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!(
                    "active-tools changes are not supported for {} tool(s)",
                    active_tool_names.len()
                ),
                None,
            ))
        })
    }
    /// Append a compaction entry.
    fn append_compaction<'a>(
        &'a self,
        summary: String,
        first_kept_entry_id: String,
        tokens_before: u64,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            let _ = (
                summary,
                first_kept_entry_id,
                tokens_before,
                details,
                from_hook,
            );
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "compaction entries are not supported",
                None,
            ))
        })
    }
    /// Append a custom data entry.
    fn append_custom_entry<'a>(
        &'a self,
        custom_type: String,
        data: Option<Value>,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            let _ = data;
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("custom entries are not supported for {custom_type}"),
                None,
            ))
        })
    }
    /// Append a custom message entry.
    fn append_custom_message_entry<'a>(
        &'a self,
        custom_type: String,
        content: CustomMessageContent,
        display: bool,
        details: Option<Value>,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            let _ = (content, display, details);
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("custom message entries are not supported for {custom_type}"),
                None,
            ))
        })
    }
    /// Append or clear a label.
    fn append_label<'a>(
        &'a self,
        target_id: String,
        label: Option<String>,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            let _ = label;
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("labels are not supported for {target_id}"),
                None,
            ))
        })
    }
    /// Append session display-name metadata.
    fn append_session_name<'a>(
        &'a self,
        name: String,
    ) -> HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("session names are not supported for {name}"),
                None,
            ))
        })
    }
    /// Move current leaf.
    fn move_to<'a>(
        &'a self,
        entry_id: Option<String>,
        summary: Option<BranchSummaryDraft>,
    ) -> HarnessFuture<'a, Result<Option<String>, SessionError>>;
}

/// Branch summary data accepted by [`Session::move_to`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummaryDraft {
    /// Summary text.
    pub summary: String,
    /// Optional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// Whether summary came from a hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_hook: Option<bool>,
}

/// Session creation options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCreateOptions {
    /// Optional session id.
    pub id: Option<String>,
}

/// Session fork options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionForkOptions {
    /// Entry id to fork from.
    pub entry_id: Option<String>,
    /// Fork position relative to the entry.
    pub position: Option<SessionForkPosition>,
    /// Optional session id.
    pub id: Option<String>,
}

/// Position used by session fork operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionForkPosition {
    /// Fork before the entry.
    Before,
    /// Fork at the entry.
    At,
}

/// Session repository contract.
pub trait SessionRepo: Send + Sync {
    /// Create a session.
    fn create<'a>(
        &'a self,
        options: SessionCreateOptions,
    ) -> HarnessFuture<'a, Result<Arc<dyn Session>, SessionError>>;
    /// Open a session by metadata.
    fn open<'a>(
        &'a self,
        metadata: SessionMetadata,
    ) -> HarnessFuture<'a, Result<Arc<dyn Session>, SessionError>>;
    /// List sessions.
    fn list<'a>(&'a self) -> HarnessFuture<'a, Result<Vec<SessionMetadata>, SessionError>>;
    /// Delete a session.
    fn delete<'a>(
        &'a self,
        metadata: SessionMetadata,
    ) -> HarnessFuture<'a, Result<(), SessionError>>;
    /// Fork a session.
    fn fork<'a>(
        &'a self,
        source: SessionMetadata,
        options: SessionForkOptions,
    ) -> HarnessFuture<'a, Result<Arc<dyn Session>, SessionError>>;
}

/// JSONL session creation options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonlSessionCreateOptions {
    /// Optional session id.
    pub id: Option<String>,
    /// Working directory.
    pub cwd: String,
    /// Optional parent session path.
    pub parent_session_path: Option<String>,
}

/// JSONL session listing options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonlSessionListOptions {
    /// Optional working directory filter.
    pub cwd: Option<String>,
}

/// Agent harness lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHarnessPhase {
    /// Idle phase.
    Idle,
    /// Agent turn phase.
    Turn,
    /// Compaction phase.
    Compaction,
    /// Branch-summary phase.
    BranchSummary,
    /// Retry phase.
    Retry,
}

/// Queue update event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueUpdateEvent {
    /// Steering queue.
    pub steer: Vec<AgentMessage>,
    /// Follow-up queue.
    pub follow_up: Vec<AgentMessage>,
    /// Next-turn queue.
    pub next_turn: Vec<AgentMessage>,
}

/// Save-point event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePointEvent {
    /// Whether there were pending mutations.
    pub had_pending_mutations: bool,
}

/// Abort event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbortEvent {
    /// Cleared steering messages.
    pub cleared_steer: Vec<AgentMessage>,
    /// Cleared follow-up messages.
    pub cleared_follow_up: Vec<AgentMessage>,
}

/// Settled event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettledEvent {
    /// Number of messages queued for the next turn.
    pub next_turn_count: usize,
}

/// Event emitted before an agent run starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeAgentStartEvent<TSkill = Skill, TPromptTemplate = PromptTemplate> {
    /// Prompt text.
    pub prompt: String,
    /// Prompt images.
    pub images: Option<Vec<ImageContent>>,
    /// System prompt.
    pub system_prompt: String,
    /// Harness resources.
    pub resources: AgentHarnessResources<TSkill, TPromptTemplate>,
}

/// Context event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextEvent {
    /// Current messages.
    pub messages: Vec<AgentMessage>,
}

/// Event emitted before a provider request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeProviderRequestEvent {
    /// Request model.
    pub model: Model,
    /// Session id.
    pub session_id: String,
    /// Stream options snapshot.
    pub stream_options: AgentHarnessStreamOptions,
}

/// Event emitted before a provider payload is sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeforeProviderPayloadEvent {
    /// Request model.
    pub model: Model,
    /// Provider payload.
    pub payload: Value,
}

/// Event emitted after provider response headers are known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AfterProviderResponseEvent {
    /// HTTP status.
    pub status: u16,
    /// Response headers.
    pub headers: HashMap<String, String>,
}

/// Tool-call hook event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallEvent {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Validated input.
    pub input: HashMap<String, Value>,
}

/// Tool-result hook event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultEvent {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Validated input.
    pub input: HashMap<String, Value>,
    /// Result content.
    pub content: Vec<AgentToolResultContent>,
    /// Result details.
    pub details: Value,
    /// Whether result is an error.
    pub is_error: bool,
}

/// Compaction settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSettings {
    /// Whether compaction is enabled.
    pub enabled: bool,
    /// Tokens reserved for response.
    pub reserve_tokens: u64,
    /// Recent tokens to keep.
    pub keep_recent_tokens: u64,
}

/// File operation sets collected from tool results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOperations {
    /// Read file paths.
    pub read: HashSet<String>,
    /// Written file paths.
    pub written: HashSet<String>,
    /// Edited file paths.
    pub edited: HashSet<String>,
}

/// Prepared compaction input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionPreparation {
    /// First entry that will be kept.
    pub first_kept_entry_id: String,
    /// Messages to summarize.
    pub messages_to_summarize: Vec<AgentMessage>,
    /// Prefix messages for split turns.
    pub turn_prefix_messages: Vec<AgentMessage>,
    /// Whether compaction split a turn.
    pub is_split_turn: bool,
    /// Token count before compaction.
    pub tokens_before: u64,
    /// Previous summary, if any.
    pub previous_summary: Option<String>,
    /// File operations observed.
    pub file_ops: FileOperations,
    /// Compaction settings used.
    pub settings: CompactionSettings,
}

/// Event emitted before session compaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeCompactEvent {
    /// Prepared compaction data.
    pub preparation: CompactionPreparation,
    /// Branch entries.
    pub branch_entries: Vec<SessionTreeEntry>,
    /// Optional custom instructions.
    pub custom_instructions: Option<String>,
}

/// Event emitted after session compaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompactEvent {
    /// Stored compaction entry.
    pub compaction_entry: CompactionEntry,
    /// Whether compaction came from hook result.
    pub from_hook: bool,
}

/// Prepared tree-navigation input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreePreparation {
    /// Target entry id.
    pub target_id: String,
    /// Previous leaf id.
    pub old_leaf_id: Option<String>,
    /// Common ancestor id.
    pub common_ancestor_id: Option<String>,
    /// Entries to summarize.
    pub entries_to_summarize: Vec<SessionTreeEntry>,
    /// Whether user requested summary.
    pub user_wants_summary: bool,
    /// Optional custom instructions.
    pub custom_instructions: Option<String>,
    /// Replace default instructions.
    pub replace_instructions: Option<bool>,
    /// Optional branch label.
    pub label: Option<String>,
}

/// Event emitted before tree navigation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionBeforeTreeEvent {
    /// Prepared tree data.
    pub preparation: TreePreparation,
}

/// Event emitted after tree navigation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTreeEvent {
    /// New leaf id.
    pub new_leaf_id: Option<String>,
    /// Old leaf id.
    pub old_leaf_id: Option<String>,
    /// Optional branch-summary entry.
    pub summary_entry: Option<BranchSummaryEntry>,
    /// Whether summary came from a hook.
    pub from_hook: Option<bool>,
}

/// Model update event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelUpdateEvent {
    /// New model.
    pub model: Model,
    /// Previous model.
    pub previous_model: Option<Model>,
    /// Update source.
    pub source: RestoreSource,
}

/// Update source for restored or explicitly set state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreSource {
    /// Explicit setter.
    Set,
    /// Restored from session.
    Restore,
}

/// Thinking-level update event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingLevelUpdateEvent {
    /// New level.
    pub level: ThinkingLevel,
    /// Previous level.
    pub previous_level: ThinkingLevel,
}

/// Tools update event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsUpdateEvent {
    /// All configured tool names.
    pub tool_names: Vec<String>,
    /// Previous configured tool names.
    pub previous_tool_names: Vec<String>,
    /// Active tool names.
    pub active_tool_names: Vec<String>,
    /// Previous active tool names.
    pub previous_active_tool_names: Vec<String>,
    /// Update source.
    pub source: RestoreSource,
}

/// Resources update event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourcesUpdateEvent<TSkill = Skill, TPromptTemplate = PromptTemplate> {
    /// New resources.
    pub resources: AgentHarnessResources<TSkill, TPromptTemplate>,
    /// Previous resources.
    pub previous_resources: AgentHarnessResources<TSkill, TPromptTemplate>,
}

/// Harness-owned event union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AgentHarnessOwnEvent<TSkill = Skill, TPromptTemplate = PromptTemplate> {
    /// Queue update event.
    QueueUpdate(QueueUpdateEvent),
    /// Save-point event.
    SavePoint(SavePointEvent),
    /// Abort event.
    Abort(AbortEvent),
    /// Settled event.
    Settled(SettledEvent),
    /// Before-agent-start event.
    BeforeAgentStart(BeforeAgentStartEvent<TSkill, TPromptTemplate>),
    /// Context event.
    Context(ContextEvent),
    /// Before-provider-request event.
    BeforeProviderRequest(BeforeProviderRequestEvent),
    /// Before-provider-payload event.
    BeforeProviderPayload(BeforeProviderPayloadEvent),
    /// After-provider-response event.
    AfterProviderResponse(AfterProviderResponseEvent),
    /// Tool-call event.
    ToolCall(ToolCallEvent),
    /// Tool-result event.
    ToolResult(ToolResultEvent),
    /// Session-before-compact event.
    SessionBeforeCompact(SessionBeforeCompactEvent),
    /// Session-compact event.
    SessionCompact(SessionCompactEvent),
    /// Session-before-tree event.
    SessionBeforeTree(SessionBeforeTreeEvent),
    /// Session-tree event.
    SessionTree(SessionTreeEvent),
    /// Model update event.
    ModelUpdate(ModelUpdateEvent),
    /// Thinking-level update event.
    ThinkingLevelUpdate(ThinkingLevelUpdateEvent),
    /// Resources update event.
    ResourcesUpdate(ResourcesUpdateEvent<TSkill, TPromptTemplate>),
    /// Tools update event.
    ToolsUpdate(ToolsUpdateEvent),
}

/// Agent or harness event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentHarnessEvent<TSkill = Skill, TPromptTemplate = PromptTemplate> {
    /// Low-level agent event.
    Agent(AgentEvent),
    /// Harness-owned event.
    Harness(AgentHarnessOwnEvent<TSkill, TPromptTemplate>),
}

/// Hook result for `before_agent_start`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BeforeAgentStartResult {
    /// Replacement messages.
    pub messages: Option<Vec<AgentMessage>>,
    /// Replacement system prompt.
    pub system_prompt: Option<String>,
}

/// Hook result for context replacement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextResult {
    /// Replacement messages.
    pub messages: Vec<AgentMessage>,
}

/// Hook result for provider request options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BeforeProviderRequestResult {
    /// Stream option patch.
    pub stream_options: Option<AgentHarnessStreamOptionsPatch>,
}

/// Hook result for provider payload replacement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeforeProviderPayloadResult {
    /// Replacement payload.
    pub payload: Value,
}

/// Hook result for tool-call preflight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Whether to block the call.
    pub block: Option<bool>,
    /// Optional block reason.
    pub reason: Option<String>,
}

/// Hook patch for tool results.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPatch {
    /// Replacement content.
    pub content: Option<Vec<AgentToolResultContent>>,
    /// Replacement details.
    pub details: Option<Value>,
    /// Replacement error flag.
    pub is_error: Option<bool>,
    /// Replacement termination hint.
    pub terminate: Option<bool>,
}

/// Hook result before session compaction.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionBeforeCompactResult {
    /// Cancel compaction.
    pub cancel: Option<bool>,
    /// Hook-provided compaction result.
    pub compaction: Option<CompactResult>,
}

/// Hook result before tree navigation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeTreeResult {
    /// Cancel navigation.
    pub cancel: Option<bool>,
    /// Hook-provided summary.
    pub summary: Option<BranchSummaryDraft>,
    /// Optional custom instructions.
    pub custom_instructions: Option<String>,
    /// Replace default instructions.
    pub replace_instructions: Option<bool>,
    /// Optional label.
    pub label: Option<String>,
}

/// Prompt options accepted by the harness.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentHarnessPromptOptions {
    /// Prompt images.
    pub images: Option<Vec<ImageContent>>,
}

/// Result returned from aborting the harness.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbortResult {
    /// Cleared steering queue.
    pub cleared_steer: Vec<AgentMessage>,
    /// Cleared follow-up queue.
    pub cleared_follow_up: Vec<AgentMessage>,
}

/// Compaction result persisted to the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResult {
    /// Summary text.
    pub summary: String,
    /// First kept entry id.
    pub first_kept_entry_id: String,
    /// Token count before compaction.
    pub tokens_before: u64,
    /// Optional details.
    pub details: Option<Value>,
}

/// Tree navigation result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateTreeResult {
    /// Whether navigation was cancelled.
    pub cancelled: bool,
    /// Optional editor text.
    pub editor_text: Option<String>,
    /// Optional summary entry.
    pub summary_entry: Option<BranchSummaryEntry>,
}

/// Branch summary generation options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateBranchSummaryOptions {
    /// Model used for summarization.
    pub model: Model,
    /// API key override.
    pub api_key: String,
    /// Optional request headers.
    pub headers: Option<HashMap<String, String>>,
    /// Optional custom instructions.
    pub custom_instructions: Option<String>,
    /// Replace default instructions.
    pub replace_instructions: Option<bool>,
    /// Reserved token budget.
    pub reserve_tokens: Option<u64>,
}

/// Branch summary result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummaryResult {
    /// Summary text.
    pub summary: String,
    /// Read file paths.
    pub read_files: Vec<String>,
    /// Modified file paths.
    pub modified_files: Vec<String>,
}

/// System prompt callback context.
pub struct SystemPromptContext<
    TSkill = Skill,
    TPromptTemplate = PromptTemplate,
    TToolDetails = Value,
> {
    /// Execution environment.
    pub env: Arc<dyn ExecutionEnv>,
    /// Session handle.
    pub session: Arc<dyn Session>,
    /// Active model.
    pub model: Model,
    /// Active thinking level.
    pub thinking_level: ThinkingLevel,
    /// Active tools.
    pub active_tools: Vec<AgentTool<TToolDetails>>,
    /// Harness resources.
    pub resources: AgentHarnessResources<TSkill, TPromptTemplate>,
}

/// System prompt callback.
pub type SystemPromptFn<TSkill = Skill, TPromptTemplate = PromptTemplate, TToolDetails = Value> =
    Arc<
        dyn Fn(
                SystemPromptContext<TSkill, TPromptTemplate, TToolDetails>,
            ) -> HarnessFuture<'static, String>
            + Send
            + Sync,
    >;

/// Static or callback system prompt.
pub enum SystemPrompt<TSkill = Skill, TPromptTemplate = PromptTemplate, TToolDetails = Value> {
    /// Static prompt text.
    Text(String),
    /// Dynamic callback.
    Callback(SystemPromptFn<TSkill, TPromptTemplate, TToolDetails>),
}

/// Agent harness construction options.
pub struct AgentHarnessOptions<
    TSkill = Skill,
    TPromptTemplate = PromptTemplate,
    TToolDetails = Value,
> {
    /// Execution environment.
    pub env: Arc<dyn ExecutionEnv>,
    /// Session handle.
    pub session: Arc<dyn Session>,
    /// Provider collection.
    pub models: Models,
    /// Available tools.
    pub tools: Option<Vec<AgentTool<TToolDetails>>>,
    /// Prompt templates and skills.
    pub resources: Option<AgentHarnessResources<TSkill, TPromptTemplate>>,
    /// Static or dynamic system prompt.
    pub system_prompt: Option<SystemPrompt<TSkill, TPromptTemplate, TToolDetails>>,
    /// Curated stream/provider request options.
    pub stream_options: Option<AgentHarnessStreamOptions>,
    /// Active model.
    pub model: Model,
    /// Active thinking level.
    pub thinking_level: Option<ThinkingLevel>,
    /// Active tool names.
    pub active_tool_names: Option<Vec<String>>,
    /// Steering queue mode.
    pub steering_mode: Option<QueueMode>,
    /// Follow-up queue mode.
    pub follow_up_mode: Option<QueueMode>,
}
