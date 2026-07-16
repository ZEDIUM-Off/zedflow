//! Shared AI types ported from Pi's `packages/ai/src/types.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Known chat API identifiers built into Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownApi {
    /// OpenAI-compatible chat completions API.
    OpenAICompletions,
    /// Mistral conversations API.
    MistralConversations,
    /// OpenAI Responses API.
    OpenAIResponses,
    /// Azure OpenAI Responses API.
    AzureOpenAIResponses,
    /// OpenAI Codex Responses API.
    OpenAICodexResponses,
    /// Anthropic Messages API.
    AnthropicMessages,
    /// AWS Bedrock Converse stream API.
    BedrockConverseStream,
    /// Google Generative AI API.
    GoogleGenerativeAI,
    /// Google Vertex AI API.
    GoogleVertex,
}

/// Chat API identifier, including custom API strings.
pub type Api = String;

/// Known image API identifiers built into Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownImagesApi {
    /// OpenRouter image generation API.
    OpenRouterImages,
}

/// Image API identifier, including custom API strings.
pub type ImagesApi = String;

/// Known provider identifiers built into Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownProvider {
    /// Amazon Bedrock provider.
    AmazonBedrock,
    /// Ant Ling provider.
    AntLing,
    /// Anthropic provider.
    Anthropic,
    /// Google provider.
    Google,
    /// Google Vertex provider.
    GoogleVertex,
    /// OpenAI provider.
    OpenAI,
    /// Azure OpenAI Responses provider.
    AzureOpenAIResponses,
    /// OpenAI Codex provider.
    OpenAICodex,
    /// NVIDIA provider.
    Nvidia,
    /// DeepSeek provider.
    Deepseek,
    /// GitHub Copilot provider.
    GithubCopilot,
    /// xAI provider.
    Xai,
    /// Groq provider.
    Groq,
    /// Cerebras provider.
    Cerebras,
    /// OpenRouter provider.
    OpenRouter,
    /// Vercel AI Gateway provider.
    VercelAIGateway,
    /// Z.ai provider.
    Zai,
    /// Z.ai Coding CN provider.
    ZaiCodingCn,
    /// Mistral provider.
    Mistral,
    /// MiniMax provider.
    Minimax,
    /// MiniMax CN provider.
    MinimaxCn,
    /// Moonshot AI provider.
    Moonshotai,
    /// Moonshot AI CN provider.
    MoonshotaiCn,
    /// Hugging Face provider.
    Huggingface,
    /// Fireworks provider.
    Fireworks,
    /// Together provider.
    Together,
    /// OpenCode provider.
    Opencode,
    /// OpenCode Go provider.
    OpencodeGo,
    /// Kimi Coding provider.
    KimiCoding,
    /// Cloudflare Workers AI provider.
    CloudflareWorkersAI,
    /// Cloudflare AI Gateway provider.
    CloudflareAIGateway,
    /// Xiaomi provider.
    Xiaomi,
    /// Xiaomi Token Plan CN provider.
    XiaomiTokenPlanCn,
    /// Xiaomi Token Plan AMS provider.
    XiaomiTokenPlanAms,
    /// Xiaomi Token Plan SGP provider.
    XiaomiTokenPlanSgp,
}

/// Provider identifier, including custom provider strings.
pub type ProviderId = String;

/// Known image provider identifiers built into Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownImagesProvider {
    /// OpenRouter image provider.
    OpenRouter,
}

/// Image provider identifier, including custom provider strings.
pub type ImagesProviderId = String;

/// Pi reasoning effort levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    XHigh,
}

/// Model thinking levels, including the `off` sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelThinkingLevel {
    /// Disable model thinking.
    Off,
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    XHigh,
}

/// Provider/model-specific values for Pi thinking levels; `None` marks unsupported levels.
pub type ThinkingLevelMap = HashMap<ModelThinkingLevel, Option<String>>;

/// Value accepted for OpenAI-compatible chat template kwargs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatTemplateKwargValue {
    /// String kwarg value.
    String(String),
    /// Numeric kwarg value.
    Number(f64),
    /// Boolean kwarg value.
    Boolean(bool),
    /// Null kwarg value.
    Null,
    /// Pi-controlled thinking variable reference.
    Variable(ChatTemplateKwargVariable),
}

/// Pi-controlled chat template variable reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTemplateKwargVariable {
    /// Variable name consumed by Pi's chat template mapping.
    #[serde(rename = "$var")]
    pub variable: ChatTemplateVariable,
    /// Whether to omit this kwarg when thinking is off.
    #[serde(rename = "omitWhenOff", skip_serializing_if = "Option::is_none")]
    pub omit_when_off: Option<bool>,
}

/// Pi thinking variables available to chat template kwargs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChatTemplateVariable {
    /// Boolean variable indicating whether thinking is enabled.
    #[serde(rename = "thinking.enabled")]
    ThinkingEnabled,
    /// Effort variable containing the mapped thinking effort.
    #[serde(rename = "thinking.effort")]
    ThinkingEffort,
}

/// Token budgets for each thinking level on token-based providers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingBudgets {
    /// Budget for minimal thinking.
    pub minimal: Option<u32>,
    /// Budget for low thinking.
    pub low: Option<u32>,
    /// Budget for medium thinking.
    pub medium: Option<u32>,
    /// Budget for high thinking.
    pub high: Option<u32>,
}

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheRetention {
    /// Disable prompt cache retention.
    None,
    /// Use short prompt cache retention.
    #[default]
    Short,
    /// Use long prompt cache retention.
    Long,
}

/// Provider transport preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    /// Server-sent events transport.
    Sse,
    /// WebSocket transport.
    Websocket,
    /// Cached WebSocket transport.
    WebsocketCached,
    /// Let the provider choose the transport.
    Auto,
}

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// Provider HTTP headers; `None` suppresses a provider/API default header.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Provider HTTP response metadata exposed to response hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// HTTP status code.
    pub status: u16,
    /// HTTP response headers.
    pub headers: HashMap<String, String>,
}

/// Rust abort signal replacing Pi's DOM `AbortSignal` stream option.
pub type AbortSignal = crate::utils::abort_signals::AbortSignal;

/// Error returned by a provider payload or response hook.
#[derive(Debug)]
pub struct ProviderHookError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ProviderHookError {
    /// Wraps a hook failure while retaining its error source.
    #[must_use]
    pub fn new(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for ProviderHookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for ProviderHookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Callback that can inspect or replace a provider payload before it is sent.
///
/// # Errors
///
/// Returns [`ProviderHookError`] when the hook rejects the request.
pub type PayloadHook<TApi = Api> = Arc<
    dyn Fn(Value, Model<TApi>) -> BoxFuture<'static, Result<Option<Value>, ProviderHookError>>
        + Send
        + Sync,
>;

/// Callback invoked after an HTTP response is received.
///
/// # Errors
///
/// Returns [`ProviderHookError`] when the hook rejects the response.
pub type ResponseHook<TApi = Api> = Arc<
    dyn Fn(ProviderResponse, Model<TApi>) -> BoxFuture<'static, Result<(), ProviderHookError>>
        + Send
        + Sync,
>;

/// Base options shared by Pi chat providers.
#[derive(Clone, Default)]
pub struct StreamOptions<TApi = Api> {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens.
    pub max_tokens: Option<u32>,
    /// Cancellation signal.
    pub signal: Option<AbortSignal>,
    /// API key override.
    pub api_key: Option<String>,
    /// Preferred transport for providers that support multiple transports.
    pub transport: Option<Transport>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional session identifier for session-aware providers.
    pub session_id: Option<String>,
    /// Optional callback for inspecting or replacing provider payloads before sending.
    pub on_payload: Option<PayloadHook<TApi>>,
    /// Optional callback invoked after an HTTP response is received.
    pub on_response: Option<ResponseHook<TApi>>,
    /// Optional custom HTTP headers to include in API requests.
    pub headers: Option<ProviderHeaders>,
    /// HTTP request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// WebSocket connect timeout in milliseconds.
    pub websocket_connect_timeout_ms: Option<u64>,
    /// Maximum client-side retry attempts.
    pub max_retries: Option<u32>,
    /// Maximum retry delay in milliseconds.
    pub max_retry_delay_ms: Option<u64>,
    /// Optional metadata to include in API requests.
    pub metadata: Option<HashMap<String, Value>>,
    /// Provider-scoped environment values.
    pub env: Option<ProviderEnv>,
    /// Extra provider-specific options from Pi's `Record<string, unknown>` intersection.
    pub extra: HashMap<String, Value>,
}

/// Provider stream options, including provider-specific unknown fields.
pub type ProviderStreamOptions = StreamOptions;

/// Type-level map marker for known API option structs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApiOptionsMap;

/// Stream options for a known or custom API.
#[derive(Clone)]
pub enum ApiStreamOptions {
    /// Anthropic Messages options.
    AnthropicMessages(crate::api::anthropic_messages::AnthropicOptions),
    /// OpenAI Chat Completions options.
    OpenAICompletions(crate::api::openai_completions::OpenAICompletionsOptions),
    /// OpenAI Responses options.
    OpenAIResponses(crate::api::openai_responses::OpenAIResponsesOptions),
    /// OpenAI Codex Responses options.
    OpenAICodexResponses(crate::api::openai_codex_responses::OpenAICodexResponsesOptions),
    /// Azure OpenAI Responses options.
    AzureOpenAIResponses(crate::api::azure_openai_responses::AzureOpenAIResponsesOptions),
    /// Google Generative AI options.
    GoogleGenerativeAI(crate::api::google_generative_ai::GoogleOptions),
    /// Google Vertex AI options.
    GoogleVertex(crate::api::google_vertex::GoogleVertexOptions),
    /// Mistral Conversations options.
    MistralConversations(crate::api::mistral_conversations::MistralOptions),
    /// Bedrock Converse stream options.
    BedrockConverseStream(crate::api::bedrock_converse_stream::BedrockOptions),
    /// Generic options for custom API strings.
    Custom(StreamOptions),
}

/// Pi assistant-message event stream imported from `utils/event-stream.ts`.
pub use crate::utils::event_stream::AssistantMessageEventStream;

/// Uniform stream contract of a chat API implementation module.
#[derive(Clone)]
pub struct ProviderStreams {
    /// Full stream function.
    pub stream: StreamFunction,
    /// Simple stream function.
    pub stream_simple: StreamFunction<Api, SimpleStreamOptions>,
}

/// Uniform contract of an image-generation API implementation module.
#[derive(Clone)]
pub struct ProviderImages {
    /// Image generation function.
    pub generate_images: ImagesFunction,
}

/// Callback that can inspect or replace an image provider payload before it is sent.
///
/// # Errors
///
/// Returns [`ProviderHookError`] when the hook rejects the request.
pub type ImagesPayloadHook<TApi = ImagesApi> = Arc<
    dyn Fn(Value, ImagesModel<TApi>) -> BoxFuture<'static, Result<Option<Value>, ProviderHookError>>
        + Send
        + Sync,
>;

/// Callback invoked after an image API HTTP response is received.
///
/// # Errors
///
/// Returns [`ProviderHookError`] when the hook rejects the response.
pub type ImagesResponseHook<TApi = ImagesApi> = Arc<
    dyn Fn(ProviderResponse, ImagesModel<TApi>) -> BoxFuture<'static, Result<(), ProviderHookError>>
        + Send
        + Sync,
>;

/// Options shared by Pi image-generation providers.
#[derive(Clone, Default)]
pub struct ImagesOptions<TApi = ImagesApi> {
    /// Cancellation signal.
    pub signal: Option<AbortSignal>,
    /// API key override.
    pub api_key: Option<String>,
    /// Provider-scoped environment values.
    pub env: Option<ProviderEnv>,
    /// Optional callback for inspecting or replacing provider payloads before sending.
    pub on_payload: Option<ImagesPayloadHook<TApi>>,
    /// Optional callback invoked after an HTTP response is received.
    pub on_response: Option<ImagesResponseHook<TApi>>,
    /// Optional custom HTTP headers.
    pub headers: Option<ProviderHeaders>,
    /// HTTP request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum client-side retry attempts.
    pub max_retries: Option<u32>,
    /// Maximum retry delay in milliseconds.
    pub max_retry_delay_ms: Option<u64>,
    /// Optional metadata to include in API requests.
    pub metadata: Option<HashMap<String, Value>>,
    /// Extra provider-specific options from Pi's `Record<string, unknown>` intersection.
    pub extra: HashMap<String, Value>,
}

/// Provider image options, including provider-specific unknown fields.
pub type ProviderImagesOptions = ImagesOptions;

/// Unified options with reasoning passed to `streamSimple()` and `completeSimple()`.
#[derive(Clone, Default)]
pub struct SimpleStreamOptions<TApi = Api> {
    /// Base stream options.
    pub stream: StreamOptions<TApi>,
    /// Unified reasoning level.
    pub reasoning: Option<ThinkingLevel>,
    /// Custom token budgets for thinking levels.
    pub thinking_budgets: Option<ThinkingBudgets>,
}

/// Generic chat stream function contract.
pub type StreamFunction<TApi = Api, TOptions = StreamOptions> = Arc<
    dyn Fn(&Model<TApi>, &Context, Option<&TOptions>) -> AssistantMessageEventStream + Send + Sync,
>;

/// Generic image generation function contract.
pub type ImagesFunction<TApi = ImagesApi, TOptions = ImagesOptions> = Arc<
    dyn Fn(
            &ImagesModel<TApi>,
            &ImagesContext,
            Option<&TOptions>,
        ) -> BoxFuture<'static, AssistantImages>
        + Send
        + Sync,
>;

/// Versioned text signature metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSignatureV1 {
    /// Signature schema version.
    pub v: u8,
    /// Provider item identifier.
    pub id: String,
    /// Optional response phase.
    pub phase: Option<TextSignaturePhase>,
}

/// Phase attached to a text signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSignaturePhase {
    /// Commentary phase.
    Commentary,
    /// Final answer phase.
    FinalAnswer,
}

/// Text content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// Content discriminator; Pi value is `text`.
    #[serde(rename = "type")]
    pub content_type: TextContentType,
    /// Text payload.
    pub text: String,
    /// Optional provider text signature.
    pub text_signature: Option<String>,
}

/// Discriminator for text content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextContentType {
    /// Text content discriminator.
    Text,
}

/// Thinking content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingContent {
    /// Content discriminator; Pi value is `thinking`.
    #[serde(rename = "type")]
    pub content_type: ThinkingContentType,
    /// Thinking text payload.
    pub thinking: String,
    /// Optional provider thinking signature.
    pub thinking_signature: Option<String>,
    /// Whether the thinking content was redacted by safety filters.
    pub redacted: Option<bool>,
}

/// Discriminator for thinking content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingContentType {
    /// Thinking content discriminator.
    Thinking,
}

/// Image content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// Content discriminator; Pi value is `image`.
    #[serde(rename = "type")]
    pub content_type: ImageContentType,
    /// Base64 encoded image data.
    pub data: String,
    /// Image MIME type.
    pub mime_type: String,
}

/// Discriminator for image content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageContentType {
    /// Image content discriminator.
    Image,
}

/// Tool call content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// Content discriminator; Pi value is `toolCall`.
    #[serde(rename = "type")]
    pub content_type: ToolCallType,
    /// Tool call identifier.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool arguments.
    pub arguments: HashMap<String, Value>,
    /// Google-specific opaque signature for reusing thought context.
    pub thought_signature: Option<String>,
}

/// Discriminator for tool-call content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallType {
    /// Tool-call content discriminator.
    ToolCall,
}

/// Token usage and cost metadata.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Prompt cache read tokens.
    pub cache_read: u64,
    /// Prompt cache write tokens.
    pub cache_write: u64,
    /// Subset of `cache_write` written with 1h retention.
    pub cache_write_1h: Option<u64>,
    /// Reasoning/thinking tokens, when reported by the provider.
    pub reasoning: Option<u64>,
    /// Total tokens.
    pub total_tokens: u64,
    /// Cost breakdown.
    pub cost: UsageCost,
}

/// Usage cost breakdown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageCost {
    /// Input token cost.
    pub input: f64,
    /// Output token cost.
    pub output: f64,
    /// Prompt cache read cost.
    pub cache_read: f64,
    /// Prompt cache write cost.
    pub cache_write: f64,
    /// Total cost.
    pub total: f64,
}

/// Chat stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Normal stop.
    Stop,
    /// Output length limit reached.
    Length,
    /// Tool use requested.
    ToolUse,
    /// Error termination.
    Error,
    /// Aborted termination.
    Aborted,
}

/// Successful stream stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DoneStopReason {
    /// Normal stop.
    Stop,
    /// Output length limit reached.
    Length,
    /// Tool use requested.
    ToolUse,
}

/// Error stream stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorStopReason {
    /// Aborted termination.
    Aborted,
    /// Error termination.
    Error,
}

/// User message role discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserMessageRole {
    /// User role.
    User,
}

/// User message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessage {
    /// Role discriminator; Pi value is `user`.
    pub role: UserMessageRole,
    /// User content.
    pub content: UserMessageContent,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// User message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserMessageContent {
    /// Plain text user content.
    Text(String),
    /// Structured user content.
    Blocks(Vec<UserContentBlock>),
}

/// Structured user content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContentBlock {
    /// Text content block.
    Text(TextContent),
    /// Image content block.
    Image(ImageContent),
}

/// Assistant message role discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantMessageRole {
    /// Assistant role.
    Assistant,
}

/// Assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    /// Role discriminator; Pi value is `assistant`.
    pub role: AssistantMessageRole,
    /// Assistant content blocks.
    pub content: Vec<AssistantContentBlock>,
    /// API identifier.
    pub api: Api,
    /// Provider identifier.
    pub provider: ProviderId,
    /// Requested model identifier.
    pub model: String,
    /// Concrete response model when different from the requested model.
    pub response_model: Option<String>,
    /// Provider-specific response/message identifier.
    pub response_id: Option<String>,
    /// Redacted provider/runtime diagnostics for failures and recoveries.
    pub diagnostics: Option<Vec<AssistantMessageDiagnostic>>,
    /// Token usage.
    pub usage: Usage,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Error message for error or aborted terminations.
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Assistant content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssistantContentBlock {
    /// Text content block.
    Text(TextContent),
    /// Thinking content block.
    Thinking(ThinkingContent),
    /// Tool-call content block.
    ToolCall(ToolCall),
}

/// Tool result message role discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolResultMessageRole {
    /// Tool-result role.
    ToolResult,
}

/// Tool result message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultMessage<TDetails = Value> {
    /// Role discriminator; Pi value is `toolResult`.
    pub role: ToolResultMessageRole,
    /// Tool call identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Result content blocks.
    pub content: Vec<ToolResultContentBlock>,
    /// Optional tool-specific details.
    pub details: Option<TDetails>,
    /// Whether the tool result represents an error.
    pub is_error: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Tool result content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContentBlock {
    /// Text content block.
    Text(TextContent),
    /// Image content block.
    Image(ImageContent),
}

/// Chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// User message.
    User(UserMessage),
    /// Assistant message.
    Assistant(AssistantMessage),
    /// Tool result message.
    ToolResult(ToolResultMessage),
}

/// Image input content.
pub type ImagesInputContent = UserContentBlock;

/// Image output content.
pub type ImagesOutputContent = ToolResultContentBlock;

/// Image generation context.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImagesContext {
    /// Image API input content.
    pub input: Vec<ImagesInputContent>,
}

/// Image generation stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImagesStopReason {
    /// Normal stop.
    Stop,
    /// Error termination.
    Error,
    /// Aborted termination.
    Aborted,
}

/// Assistant image generation result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantImages {
    /// Image API identifier.
    pub api: ImagesApi,
    /// Image provider identifier.
    pub provider: ImagesProviderId,
    /// Requested model identifier.
    pub model: String,
    /// Output content blocks.
    pub output: Vec<ImagesOutputContent>,
    /// Provider-specific response/message identifier.
    pub response_id: Option<String>,
    /// Optional token usage.
    pub usage: Option<Usage>,
    /// Stop reason.
    pub stop_reason: ImagesStopReason,
    /// Error message for error or aborted terminations.
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Tool parameter schema represented as JSON Schema.
///
/// Pi accepts TypeBox `TSchema` values here. Rust stores the serialized schema directly as
/// [`serde_json::Value`]; validation/coercion live in `utils::validation`, and schema helper
/// constructors live in `utils::typebox_helpers`.
pub type ToolParametersSchema = Value;

/// Tool definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool<TParameters = ToolParametersSchema> {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool parameter schema.
    pub parameters: TParameters,
}

/// Chat context.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Optional system prompt.
    pub system_prompt: Option<String>,
    /// Prior messages.
    pub messages: Vec<Message>,
    /// Available tools.
    pub tools: Option<Vec<Tool>>,
}

/// Event emitted by an [`AssistantMessageEventStream`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AssistantMessageEvent {
    /// Stream start event.
    Start {
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Text content start event.
    TextStart {
        /// Content block index.
        content_index: usize,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Text delta event.
    TextDelta {
        /// Content block index.
        content_index: usize,
        /// Delta text.
        delta: String,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Text end event.
    TextEnd {
        /// Content block index.
        content_index: usize,
        /// Final text content.
        content: String,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Thinking content start event.
    ThinkingStart {
        /// Content block index.
        content_index: usize,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Thinking delta event.
    ThinkingDelta {
        /// Content block index.
        content_index: usize,
        /// Delta thinking text.
        delta: String,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Thinking end event.
    ThinkingEnd {
        /// Content block index.
        content_index: usize,
        /// Final thinking content.
        content: String,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Tool-call content start event.
    ToolcallStart {
        /// Content block index.
        content_index: usize,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Tool-call delta event.
    ToolcallDelta {
        /// Content block index.
        content_index: usize,
        /// Delta tool-call text.
        delta: String,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Tool-call end event.
    ToolcallEnd {
        /// Content block index.
        content_index: usize,
        /// Final tool call.
        tool_call: ToolCall,
        /// Partial assistant message.
        partial: AssistantMessage,
    },
    /// Successful terminal event.
    Done {
        /// Successful terminal reason.
        reason: DoneStopReason,
        /// Final assistant message.
        message: AssistantMessage,
    },
    /// Error terminal event.
    Error {
        /// Error terminal reason.
        reason: ErrorStopReason,
        /// Final assistant message containing error details.
        error: AssistantMessage,
    },
}

/// Compatibility settings for OpenAI-compatible completions APIs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAICompletionsCompat {
    /// Whether the provider supports the `store` field.
    pub supports_store: Option<bool>,
    /// Whether the provider supports the `developer` role.
    pub supports_developer_role: Option<bool>,
    /// Whether the provider supports `reasoning_effort`.
    pub supports_reasoning_effort: Option<bool>,
    /// Whether the provider supports streaming usage.
    pub supports_usage_in_streaming: Option<bool>,
    /// Which field to use for max tokens.
    pub max_tokens_field: Option<MaxTokensField>,
    /// Whether tool results require the `name` field.
    pub requires_tool_result_name: Option<bool>,
    /// Whether a user message after tool results requires an assistant message in between.
    pub requires_assistant_after_tool_result: Option<bool>,
    /// Whether thinking blocks must be converted to text blocks.
    pub requires_thinking_as_text: Option<bool>,
    /// Whether replayed assistant messages must include empty reasoning content.
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    /// Format for reasoning/thinking parameters.
    pub thinking_format: Option<ThinkingFormat>,
    /// Chat template kwargs for `chat-template` thinking format.
    pub chat_template_kwargs: Option<HashMap<String, ChatTemplateKwargValue>>,
    /// OpenRouter-compatible routing preferences.
    pub open_router_routing: Option<OpenRouterRouting>,
    /// Vercel AI Gateway routing preferences.
    pub vercel_gateway_routing: Option<VercelGatewayRouting>,
    /// Whether z.ai supports top-level `tool_stream: true`.
    pub zai_tool_stream: Option<bool>,
    /// Whether the provider supports strict tool definitions.
    pub supports_strict_mode: Option<bool>,
    /// Cache control convention for prompt caching.
    pub cache_control_format: Option<CacheControlFormat>,
    /// Whether to send known session-affinity headers.
    pub send_session_affinity_headers: Option<bool>,
    /// Whether the provider supports long prompt cache retention.
    pub supports_long_cache_retention: Option<bool>,
}

/// Max-token request field name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    /// `max_completion_tokens` field.
    MaxCompletionTokens,
    /// `max_tokens` field.
    MaxTokens,
}

/// Reasoning/thinking request format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThinkingFormat {
    /// OpenAI `reasoning_effort` format.
    OpenAI,
    /// OpenRouter `reasoning: { effort }` format.
    OpenRouter,
    /// DeepSeek thinking format.
    Deepseek,
    /// Together thinking format.
    Together,
    /// Z.ai thinking format.
    Zai,
    /// Qwen top-level `enable_thinking` format.
    Qwen,
    /// Configurable chat-template kwargs format.
    ChatTemplate,
    /// Qwen chat-template kwargs format.
    QwenChatTemplate,
    /// Top-level string thinking format.
    StringThinking,
    /// Ant Ling reasoning format.
    AntLing,
}

/// Prompt cache-control convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheControlFormat {
    /// Anthropic-style cache control markers.
    Anthropic,
}

/// Compatibility settings for OpenAI Responses APIs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIResponsesCompat {
    /// Whether the provider supports the `developer` role.
    pub supports_developer_role: Option<bool>,
    /// Whether to send the OpenAI `session_id` cache-affinity header.
    pub send_session_id_header: Option<bool>,
    /// Whether the provider supports long prompt cache retention.
    pub supports_long_cache_retention: Option<bool>,
}

/// Compatibility settings for Anthropic Messages-compatible APIs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessagesCompat {
    /// Whether the provider accepts per-tool `eager_input_streaming`.
    pub supports_eager_tool_input_streaming: Option<bool>,
    /// Whether the provider supports Anthropic long cache retention.
    pub supports_long_cache_retention: Option<bool>,
    /// Whether to send `x-session-affinity` from `options.sessionId`.
    pub send_session_affinity_headers: Option<bool>,
    /// Whether the provider supports Anthropic-style cache control on tools.
    pub supports_cache_control_on_tools: Option<bool>,
    /// Whether the model accepts the Anthropic `temperature` request field.
    pub supports_temperature: Option<bool>,
    /// Whether to force adaptive thinking.
    pub force_adaptive_thinking: Option<bool>,
    /// Whether to replay empty thinking signatures as `signature: ""`.
    pub allow_empty_signature: Option<bool>,
}

/// OpenRouter provider routing preferences.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterRouting {
    /// Whether to allow backup providers.
    pub allow_fallbacks: Option<bool>,
    /// Whether to require providers to support all parameters in the request.
    pub require_parameters: Option<bool>,
    /// Data collection setting.
    pub data_collection: Option<DataCollection>,
    /// Whether to restrict routing to Zero Data Retention endpoints.
    pub zdr: Option<bool>,
    /// Whether to restrict routing to models that allow text distillation.
    pub enforce_distillable_text: Option<bool>,
    /// Ordered provider names/slugs to try.
    pub order: Option<Vec<String>>,
    /// Provider names/slugs to exclusively allow.
    pub only: Option<Vec<String>>,
    /// Provider names/slugs to skip.
    pub ignore: Option<Vec<String>>,
    /// Quantization levels to filter providers by.
    pub quantizations: Option<Vec<String>>,
    /// Sorting strategy.
    pub sort: Option<SortStrategy>,
    /// Maximum price per million units.
    pub max_price: Option<MaxPrice>,
    /// Preferred minimum throughput.
    pub preferred_min_throughput: Option<PercentilePreference>,
    /// Preferred maximum latency.
    pub preferred_max_latency: Option<PercentilePreference>,
}

/// OpenRouter data collection setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataCollection {
    /// Deny providers that may collect data.
    Deny,
    /// Allow providers that may collect data.
    Allow,
}

/// OpenRouter sorting strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SortStrategy {
    /// String sort key.
    Key(String),
    /// Structured sort options.
    Options(SortOptions),
}

/// Structured OpenRouter sort options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortOptions {
    /// Sorting metric.
    pub by: Option<String>,
    /// Partitioning strategy.
    pub partition: Option<String>,
}

/// Maximum OpenRouter price filters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MaxPrice {
    /// Price per million prompt tokens.
    pub prompt: Option<NumberOrString>,
    /// Price per million completion tokens.
    pub completion: Option<NumberOrString>,
    /// Price per image.
    pub image: Option<NumberOrString>,
    /// Price per audio unit.
    pub audio: Option<NumberOrString>,
    /// Price per request.
    pub request: Option<NumberOrString>,
}

/// Number-or-string union used by OpenRouter price filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NumberOrString {
    /// Numeric value.
    Number(f64),
    /// String value.
    String(String),
}

/// Percentile preference accepted by OpenRouter routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PercentilePreference {
    /// Single value applied to p50.
    Value(f64),
    /// Percentile-specific cutoffs.
    Percentiles(PercentileCutoffs),
}

/// Percentile-specific routing cutoffs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PercentileCutoffs {
    /// 50th percentile cutoff.
    pub p50: Option<f64>,
    /// 75th percentile cutoff.
    pub p75: Option<f64>,
    /// 90th percentile cutoff.
    pub p90: Option<f64>,
    /// 99th percentile cutoff.
    pub p99: Option<f64>,
}

/// Vercel AI Gateway routing preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VercelGatewayRouting {
    /// Provider slugs to exclusively use.
    pub only: Option<Vec<String>>,
    /// Provider slugs to try in order.
    pub order: Option<Vec<String>>,
}

/// Compatibility overrides for model APIs that support them.
#[allow(
    clippy::large_enum_variant,
    reason = "preserve the canonical Pi compatibility shape"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelCompat {
    /// OpenAI-compatible chat completions overrides.
    OpenAICompletions(OpenAICompletionsCompat),
    /// OpenAI Responses overrides.
    OpenAIResponses(OpenAIResponsesCompat),
    /// Anthropic Messages-compatible overrides.
    AnthropicMessages(AnthropicMessagesCompat),
}

/// Model cost metadata in dollars per million tokens.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCost {
    /// Input token cost.
    pub input: f64,
    /// Output token cost.
    pub output: f64,
    /// Prompt cache read cost.
    pub cache_read: f64,
    /// Prompt cache write cost.
    pub cache_write: f64,
}

/// Model input modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    /// Text input.
    Text,
    /// Image input.
    Image,
}

/// Model output modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelOutput {
    /// Text output.
    Text,
    /// Image output.
    Image,
}

/// Model metadata for the unified model system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model<TApi = Api> {
    /// Model identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// API identifier.
    pub api: TApi,
    /// Provider identifier.
    pub provider: ProviderId,
    /// Provider base URL.
    pub base_url: String,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Provider/model-specific values for Pi thinking levels.
    pub thinking_level_map: Option<ThinkingLevelMap>,
    /// Supported input modalities.
    pub input: Vec<ModelInput>,
    /// Cost metadata.
    pub cost: ModelCost,
    /// Context window size.
    pub context_window: u64,
    /// Maximum output tokens.
    pub max_tokens: u64,
    /// Default headers.
    pub headers: Option<HashMap<String, String>>,
    /// Compatibility overrides.
    pub compat: Option<ModelCompat>,
}

impl<TApi: Default> Default for Model<TApi> {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            api: TApi::default(),
            provider: String::new(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }
}

/// Image model metadata for the unified model system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesModel<TApi = ImagesApi> {
    /// Model identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Image API identifier.
    pub api: TApi,
    /// Image provider identifier.
    pub provider: ImagesProviderId,
    /// Provider base URL.
    pub base_url: String,
    /// Supported input modalities.
    pub input: Vec<ModelInput>,
    /// Supported output modalities.
    pub output: Vec<ModelOutput>,
    /// Cost metadata.
    pub cost: ModelCost,
    /// Default headers.
    pub headers: Option<HashMap<String, String>>,
}

/// Error info attached to assistant diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticErrorInfo {
    /// Error name.
    pub name: Option<String>,
    /// Error message.
    pub message: String,
    /// Error stack.
    pub stack: Option<String>,
    /// Error code.
    pub code: Option<DiagnosticErrorCode>,
}

/// String-or-number diagnostic error code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticErrorCode {
    /// String error code.
    String(String),
    /// Numeric error code.
    Number(f64),
}

/// Assistant message diagnostic entry imported by Pi `types.ts` from `utils/diagnostics.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    /// Diagnostic type.
    #[serde(rename = "type")]
    pub diagnostic_type: String,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    /// Optional error details.
    pub error: Option<DiagnosticErrorInfo>,
    /// Optional diagnostic details.
    pub details: Option<HashMap<String, Value>>,
}
