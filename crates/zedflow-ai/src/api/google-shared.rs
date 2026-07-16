//! Shared Google API conversion helpers ported from Pi.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Google API variants supported by Pi's shared Google helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoogleApiType {
    /// Google Generative AI API.
    GoogleGenerativeAi,
    /// Google Vertex AI API.
    GoogleVertex,
}

impl GoogleApiType {
    /// Returns Pi's string identifier for this Google API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoogleGenerativeAi => "google-generative-ai",
            Self::GoogleVertex => "google-vertex",
        }
    }
}

/// Thinking level for Gemini 3 models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoogleThinkingLevel {
    /// Google's unspecified thinking level.
    ThinkingLevelUnspecified,
    /// Minimal thinking level.
    Minimal,
    /// Low thinking level.
    Low,
    /// Medium thinking level.
    Medium,
    /// High thinking level.
    High,
}

impl GoogleThinkingLevel {
    /// Returns Google's enum string for this thinking level.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThinkingLevelUnspecified => "THINKING_LEVEL_UNSPECIFIED",
            Self::Minimal => "MINIMAL",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

/// Google SDK function calling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FunctionCallingConfigMode {
    /// Let the model decide whether to call functions.
    Auto,
    /// Disable function calls.
    None,
    /// Require a function call.
    Any,
}

/// Google SDK finish reasons mapped by Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinishReason {
    /// Natural stop.
    Stop,
    /// Maximum token limit reached.
    MaxTokens,
    /// Blocklist stop.
    Blocklist,
    /// Prohibited content stop.
    ProhibitedContent,
    /// Sensitive personally identifiable information stop.
    Spii,
    /// Safety stop.
    Safety,
    /// Image safety stop.
    ImageSafety,
    /// Image prohibited content stop.
    ImageProhibitedContent,
    /// Image recitation stop.
    ImageRecitation,
    /// Other image stop.
    ImageOther,
    /// Recitation stop.
    Recitation,
    /// Unspecified finish reason.
    FinishReasonUnspecified,
    /// Other finish reason.
    Other,
    /// Language stop.
    Language,
    /// Malformed function call stop.
    MalformedFunctionCall,
    /// Unexpected tool call stop.
    UnexpectedToolCall,
    /// No image stop.
    NoImage,
}

/// Pi stop reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Successful stop.
    Stop,
    /// Length-limited stop.
    Length,
    /// Tool-use stop.
    ToolUse,
    /// Provider or protocol error.
    Error,
    /// Aborted request.
    Aborted,
}

/// Model input modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelInput {
    /// Text input.
    Text,
    /// Image input.
    Image,
}

/// Minimal Pi model shape consumed by the Google shared helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model identifier sent to the provider.
    pub id: String,
    /// API identifier for same-model replay checks.
    pub api: String,
    /// Provider identifier for same-provider replay checks.
    pub provider: String,
    /// Input modalities supported by the model.
    pub input: Vec<ModelInput>,
}

/// Conversation context consumed by [`convert_messages`].
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    /// Conversation messages.
    pub messages: Vec<Message>,
}

/// Minimal Pi message shape consumed by the Google shared helpers.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// User-authored message.
    User {
        /// User content.
        content: UserContent,
    },
    /// Assistant-authored message.
    Assistant {
        /// Assistant content blocks.
        content: Vec<AssistantContent>,
        /// API that produced the message.
        api: String,
        /// Provider that produced the message.
        provider: String,
        /// Model that produced the message.
        model: String,
        /// Stop reason for replay filtering.
        stop_reason: StopReason,
    },
    /// Tool result message.
    ToolResult {
        /// Tool call identifier.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Tool result content.
        content: Vec<UserContentPart>,
        /// Whether this result represents a tool error.
        is_error: bool,
    },
}

/// Pi user message content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content.
    Parts(Vec<UserContentPart>),
}

/// Pi text or image content part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserContentPart {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Image content part.
    Image {
        /// Base64 image bytes.
        data: String,
        /// Image MIME type.
        mime_type: String,
    },
}

/// Pi assistant content block.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantContent {
    /// Text block.
    Text {
        /// Text payload.
        text: String,
        /// Optional provider-specific text signature.
        text_signature: Option<String>,
    },
    /// Thinking block.
    Thinking {
        /// Thinking payload.
        thinking: String,
        /// Optional provider-specific thinking signature.
        thinking_signature: Option<String>,
        /// Whether the thinking payload is an opaque redacted value.
        redacted: bool,
    },
    /// Tool call block.
    ToolCall {
        /// Tool call identifier.
        id: String,
        /// Function name.
        name: String,
        /// Function arguments.
        arguments: Option<Value>,
        /// Optional Google thought signature.
        thought_signature: Option<String>,
    },
}

/// Google `Content` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    /// Google role, usually `user` or `model`.
    pub role: String,
    /// Content parts.
    pub parts: Vec<Part>,
}

/// Google `Part` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Part {
    /// Text payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Google thought marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    /// Google thought signature.
    #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
    /// Inline data payload.
    #[serde(rename = "inlineData", skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    /// Function call payload.
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    /// Function response payload.
    #[serde(rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
}

/// Google inline data payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineData {
    /// MIME type.
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// Base64 data.
    pub data: String,
}

/// Google function call payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name.
    pub name: String,
    /// Function arguments.
    pub args: Value,
    /// Optional function call identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Google function response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionResponse {
    /// Function name.
    pub name: String,
    /// Function response object.
    pub response: Value,
    /// Optional multimodal response parts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<Part>>,
    /// Optional function call identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Pi tool declaration consumed by [`convert_tools`].
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool parameters schema.
    pub parameters: Value,
}

/// Google tool declaration group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDeclarationGroup {
    /// Function declarations.
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Google function declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    /// Function name.
    pub name: String,
    /// Function description.
    pub description: String,
    /// Legacy OpenAPI parameters schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    /// JSON Schema parameters schema.
    #[serde(
        rename = "parametersJsonSchema",
        skip_serializing_if = "Option::is_none"
    )]
    pub parameters_json_schema: Option<Value>,
}

/// Google streamed response chunk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentChunk {
    /// Provider response identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    /// Candidate response deltas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<Candidate>,
    /// Provider usage metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

/// Google candidate response chunk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Candidate content parts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,
    /// Google finish reason string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Google token usage metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Prompt token count, including cached content.
    #[serde(default)]
    pub prompt_token_count: u64,
    /// Output token count.
    #[serde(default)]
    pub candidates_token_count: u64,
    /// Cached prompt token count.
    #[serde(default)]
    pub cached_content_token_count: u64,
    /// Thinking token count.
    #[serde(default)]
    pub thoughts_token_count: u64,
    /// Total provider token count.
    #[serde(default)]
    pub total_token_count: u64,
}

/// Pi-compatible Google usage metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoogleUsage {
    /// Non-cached prompt tokens.
    pub input: u64,
    /// Output plus thinking tokens.
    pub output: u64,
    /// Cached prompt tokens read by Google.
    pub cache_read: u64,
    /// Cache write tokens; Google does not expose writes here.
    pub cache_write: u64,
    /// Thinking tokens.
    pub reasoning: u64,
    /// Provider total token count.
    pub total_tokens: u64,
}

/// Google streamed assistant content.
#[derive(Debug, Clone, PartialEq)]
pub enum GoogleContentBlock {
    /// Text block plus optional thought signature.
    Text {
        /// Text payload.
        text: String,
        /// Google thought signature attached to the text part.
        text_signature: Option<String>,
    },
    /// Thinking block plus optional thought signature.
    Thinking {
        /// Thinking payload.
        thinking: String,
        /// Google thought signature attached to the thinking part.
        thinking_signature: Option<String>,
    },
    /// Tool call block.
    ToolCall {
        /// Pi tool-call id.
        id: String,
        /// Function name.
        name: String,
        /// Function arguments.
        arguments: Value,
        /// Google thought signature attached to the tool call.
        thought_signature: Option<String>,
    },
}

/// Pi-compatible Google assistant message assembled from chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct GoogleAssistantMessage {
    /// API identifier.
    pub api: String,
    /// Provider identifier.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// First response id observed in the stream.
    pub response_id: Option<String>,
    /// Streamed content blocks.
    pub content: Vec<GoogleContentBlock>,
    /// Token usage metadata.
    pub usage: GoogleUsage,
    /// Pi stop reason.
    pub stop_reason: StopReason,
}

/// Deterministic event names emitted by the local Google chunk collector.
#[derive(Debug, Clone, PartialEq)]
pub enum GoogleStreamEvent {
    /// Stream started.
    Start,
    /// Text block started.
    TextStart { content_index: usize },
    /// Text delta.
    TextDelta { content_index: usize, delta: String },
    /// Text block ended.
    TextEnd {
        content_index: usize,
        content: String,
    },
    /// Thinking block started.
    ThinkingStart { content_index: usize },
    /// Thinking delta.
    ThinkingDelta { content_index: usize, delta: String },
    /// Thinking block ended.
    ThinkingEnd {
        content_index: usize,
        content: String,
    },
    /// Tool call started.
    ToolcallStart { content_index: usize },
    /// Tool-call arguments delta.
    ToolcallDelta { content_index: usize, delta: String },
    /// Tool call ended.
    ToolcallEnd { content_index: usize },
    /// Stream completed.
    Done { reason: StopReason },
}

/// In-memory Google stream collected from deterministic chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct GoogleAssistantMessageEventStream {
    /// Emitted events.
    pub events: Vec<GoogleStreamEvent>,
    /// Final assistant message.
    pub message: GoogleAssistantMessage,
}

/// Determines whether a streamed Gemini [`Part`] should be treated as thinking.
#[must_use]
pub fn is_thinking_part(part: &Part) -> bool {
    part.thought == Some(true)
}

/// Retains thought signatures during streaming.
#[must_use]
pub fn retain_thought_signature<'a>(
    existing: Option<&'a str>,
    incoming: Option<&'a str>,
) -> Option<&'a str> {
    incoming
        .filter(|signature| !signature.is_empty())
        .or(existing)
}

/// Returns true when Google APIs require explicit tool call IDs for this model.
#[must_use]
pub fn requires_tool_call_id(model_id: &str) -> bool {
    model_id.starts_with("claude-") || model_id.starts_with("gpt-oss-")
}

/// Converts internal Pi messages to Gemini `Content[]` format.
#[must_use]
pub fn convert_messages(model: &Model, context: &Context) -> Vec<Content> {
    let mut contents = Vec::new();
    let transformed_messages = transform_messages(&context.messages, model);

    for msg in transformed_messages {
        match msg {
            Message::User { content } => match content {
                UserContent::Text(text) => contents.push(Content {
                    role: "user".to_string(),
                    parts: vec![Part {
                        text: Some(sanitize_surrogates(&text)),
                        ..Part::default()
                    }],
                }),
                UserContent::Parts(items) => {
                    let parts: Vec<_> = items.into_iter().map(user_part_to_google_part).collect();
                    if !parts.is_empty() {
                        contents.push(Content {
                            role: "user".to_string(),
                            parts,
                        });
                    }
                }
            },
            Message::Assistant {
                content,
                provider,
                model: source_model,
                ..
            } => {
                let is_same_provider_and_model =
                    provider == model.provider && source_model == model.id;
                let mut parts = Vec::new();

                for block in content {
                    match block {
                        AssistantContent::Text {
                            text,
                            text_signature,
                        } => {
                            if text.trim().is_empty() {
                                continue;
                            }
                            parts.push(Part {
                                text: Some(sanitize_surrogates(&text)),
                                thought_signature: resolve_thought_signature(
                                    is_same_provider_and_model,
                                    text_signature.as_deref(),
                                )
                                .map(str::to_string),
                                ..Part::default()
                            });
                        }
                        AssistantContent::Thinking {
                            thinking,
                            thinking_signature,
                            ..
                        } => {
                            if thinking.trim().is_empty() {
                                continue;
                            }
                            if is_same_provider_and_model {
                                parts.push(Part {
                                    thought: Some(true),
                                    text: Some(sanitize_surrogates(&thinking)),
                                    thought_signature: resolve_thought_signature(
                                        is_same_provider_and_model,
                                        thinking_signature.as_deref(),
                                    )
                                    .map(str::to_string),
                                    ..Part::default()
                                });
                            } else {
                                parts.push(Part {
                                    text: Some(sanitize_surrogates(&thinking)),
                                    ..Part::default()
                                });
                            }
                        }
                        AssistantContent::ToolCall {
                            id,
                            name,
                            arguments,
                            thought_signature,
                        } => parts.push(Part {
                            thought_signature: resolve_thought_signature(
                                is_same_provider_and_model,
                                thought_signature.as_deref(),
                            )
                            .map(str::to_string),
                            function_call: Some(FunctionCall {
                                name,
                                args: arguments.unwrap_or_else(|| json!({})),
                                id: requires_tool_call_id(&model.id).then_some(id),
                            }),
                            ..Part::default()
                        }),
                    }
                }

                if !parts.is_empty() {
                    contents.push(Content {
                        role: "model".to_string(),
                        parts,
                    });
                }
            }
            Message::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
            } => {
                let text_result = content
                    .iter()
                    .filter_map(|item| match item {
                        UserContentPart::Text { text } => Some(text.as_str()),
                        UserContentPart::Image { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let image_content: Vec<_> = if model.input.contains(&ModelInput::Image) {
                    content
                        .iter()
                        .filter_map(|item| match item {
                            UserContentPart::Image { data, mime_type } => Some((data, mime_type)),
                            UserContentPart::Text { .. } => None,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let has_text = !text_result.is_empty();
                let has_images = !image_content.is_empty();
                let response_value = if has_text {
                    sanitize_surrogates(&text_result)
                } else if has_images {
                    "(see attached image)".to_string()
                } else {
                    String::new()
                };
                let image_parts: Vec<_> = image_content
                    .into_iter()
                    .map(|(data, mime_type)| Part {
                        inline_data: Some(InlineData {
                            mime_type: mime_type.clone(),
                            data: data.clone(),
                        }),
                        ..Part::default()
                    })
                    .collect();
                let supports_multimodal_function_response =
                    supports_multimodal_function_response(&model.id);
                let response_key = if is_error { "error" } else { "output" };
                let function_response_part = Part {
                    function_response: Some(FunctionResponse {
                        name: tool_name,
                        response: json!({ response_key: response_value }),
                        parts: (has_images && supports_multimodal_function_response)
                            .then(|| image_parts.clone()),
                        id: requires_tool_call_id(&model.id).then_some(tool_call_id),
                    }),
                    ..Part::default()
                };

                if let Some(last_content) = contents.last_mut()
                    && last_content.role == "user"
                    && last_content
                        .parts
                        .iter()
                        .any(|part| part.function_response.is_some())
                {
                    last_content.parts.push(function_response_part);
                } else {
                    contents.push(Content {
                        role: "user".to_string(),
                        parts: vec![function_response_part],
                    });
                }

                if has_images && !supports_multimodal_function_response {
                    let mut parts = vec![Part {
                        text: Some("Tool result image:".to_string()),
                        ..Part::default()
                    }];
                    parts.extend(image_parts);
                    contents.push(Content {
                        role: "user".to_string(),
                        parts,
                    });
                }
            }
        }
    }

    contents
}

/// Converts tools to Gemini function declarations format.
#[must_use]
pub fn convert_tools(tools: &[Tool], use_parameters: bool) -> Option<Vec<ToolDeclarationGroup>> {
    if tools.is_empty() {
        return None;
    }

    Some(vec![ToolDeclarationGroup {
        function_declarations: tools
            .iter()
            .map(|tool| {
                let parameters = if use_parameters {
                    Some(sanitize_for_open_api(&tool.parameters))
                } else {
                    None
                };
                let parameters_json_schema = if use_parameters {
                    None
                } else {
                    Some(tool.parameters.clone())
                };
                FunctionDeclaration {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters,
                    parameters_json_schema,
                }
            })
            .collect(),
    }])
}

/// Maps a tool-choice string to Gemini [`FunctionCallingConfigMode`].
#[must_use]
pub fn map_tool_choice(choice: &str) -> FunctionCallingConfigMode {
    match choice {
        "none" => FunctionCallingConfigMode::None,
        "any" => FunctionCallingConfigMode::Any,
        "auto" => FunctionCallingConfigMode::Auto,
        _ => FunctionCallingConfigMode::Auto,
    }
}

/// Maps Gemini [`FinishReason`] to Pi's [`StopReason`].
#[must_use]
pub fn map_stop_reason(reason: FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::Stop,
        FinishReason::MaxTokens => StopReason::Length,
        FinishReason::Blocklist
        | FinishReason::ProhibitedContent
        | FinishReason::Spii
        | FinishReason::Safety
        | FinishReason::ImageSafety
        | FinishReason::ImageProhibitedContent
        | FinishReason::ImageRecitation
        | FinishReason::ImageOther
        | FinishReason::Recitation
        | FinishReason::FinishReasonUnspecified
        | FinishReason::Other
        | FinishReason::Language
        | FinishReason::MalformedFunctionCall
        | FinishReason::UnexpectedToolCall
        | FinishReason::NoImage => StopReason::Error,
    }
}

/// Maps a string finish reason from raw API responses to Pi's [`StopReason`].
#[must_use]
pub fn map_stop_reason_string(reason: &str) -> StopReason {
    match reason {
        "STOP" => StopReason::Stop,
        "MAX_TOKENS" => StopReason::Length,
        _ => StopReason::Error,
    }
}

/// Maps a finish reason while preserving Pi's tool-use override.
#[must_use]
pub fn map_stop_reason_with_tool_use(reason: &str, has_tool_calls: bool) -> StopReason {
    if has_tool_calls {
        StopReason::ToolUse
    } else {
        map_stop_reason_string(reason)
    }
}

/// Converts Google usage metadata into Pi usage counters.
#[must_use]
pub fn map_usage_metadata(usage: &UsageMetadata) -> GoogleUsage {
    let cache_read = usage.cached_content_token_count;
    GoogleUsage {
        input: usage.prompt_token_count.saturating_sub(cache_read),
        output: usage.candidates_token_count + usage.thoughts_token_count,
        cache_read,
        cache_write: 0,
        reasoning: usage.thoughts_token_count,
        total_tokens: usage.total_token_count,
    }
}

/// Collects Google streaming chunks into Pi-compatible blocks/events.
#[must_use]
pub fn collect_google_stream(
    api: impl Into<String>,
    provider: impl Into<String>,
    model: impl Into<String>,
    chunks: &[GenerateContentChunk],
    id_timestamp_ms: u64,
) -> GoogleAssistantMessageEventStream {
    let mut collector =
        GoogleStreamCollector::new(api.into(), provider.into(), model.into(), id_timestamp_ms);
    for chunk in chunks {
        collector.apply_chunk(chunk);
    }
    collector.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentBlockKind {
    Text,
    Thinking,
}

/// A Google event paired with the exact partial message at emission time.
#[derive(Debug, Clone, PartialEq)]
pub struct GoogleStreamFrame {
    /// Stream event.
    pub event: GoogleStreamEvent,
    /// Progressive message snapshot for this event.
    pub partial: GoogleAssistantMessage,
}

/// Incremental Google chunk collector shared by Generative AI and Vertex transports.
pub struct GoogleStreamCollector {
    message: GoogleAssistantMessage,
    events: Vec<GoogleStreamEvent>,
    frames: Vec<GoogleStreamFrame>,
    current_block: Option<CurrentBlockKind>,
    tool_call_counter: u64,
    id_timestamp_ms: u64,
}

impl GoogleStreamCollector {
    /// Creates a collector and records its empty start snapshot.
    #[must_use]
    pub fn new(api: String, provider: String, model: String, id_timestamp_ms: u64) -> Self {
        let message = GoogleAssistantMessage {
            api,
            provider,
            model,
            response_id: None,
            content: Vec::new(),
            usage: GoogleUsage::default(),
            stop_reason: StopReason::Stop,
        };
        Self {
            frames: vec![GoogleStreamFrame {
                event: GoogleStreamEvent::Start,
                partial: message.clone(),
            }],
            message,
            events: vec![GoogleStreamEvent::Start],
            current_block: None,
            tool_call_counter: 0,
            id_timestamp_ms,
        }
    }

    /// Applies one provider response chunk without closing the current block.
    pub fn apply_chunk(&mut self, chunk: &GenerateContentChunk) {
        if self.message.response_id.is_none() {
            self.message.response_id = chunk.response_id.clone().filter(|id| !id.is_empty());
        }

        if let Some(candidate) = chunk.candidates.first() {
            if let Some(content) = &candidate.content {
                for part in &content.parts {
                    self.apply_part(part);
                }
            }
            if let Some(reason) = &candidate.finish_reason {
                self.message.stop_reason =
                    map_stop_reason_with_tool_use(reason, self.has_tool_calls());
            }
        }

        if let Some(usage) = &chunk.usage_metadata {
            self.message.usage = map_usage_metadata(usage);
        }
    }

    fn apply_part(&mut self, part: &Part) {
        if let Some(text) = &part.text {
            self.apply_text_part(text, part);
        }
        if let Some(function_call) = &part.function_call {
            self.close_current_block();
            self.apply_function_call(function_call, part.thought_signature.clone());
        }
    }

    fn apply_text_part(&mut self, text: &str, part: &Part) {
        let is_thinking = is_thinking_part(part);
        let expected_kind = if is_thinking {
            CurrentBlockKind::Thinking
        } else {
            CurrentBlockKind::Text
        };
        if self.current_block != Some(expected_kind) {
            self.close_current_block();
            self.current_block = Some(expected_kind);
            let content_index = self.message.content.len();
            if is_thinking {
                self.message.content.push(GoogleContentBlock::Thinking {
                    thinking: String::new(),
                    thinking_signature: None,
                });
                self.push_event(GoogleStreamEvent::ThinkingStart { content_index });
            } else {
                self.message.content.push(GoogleContentBlock::Text {
                    text: String::new(),
                    text_signature: None,
                });
                self.push_event(GoogleStreamEvent::TextStart { content_index });
            }
        }

        let content_index = self.message.content.len() - 1;
        match self
            .message
            .content
            .last_mut()
            .expect("current block exists")
        {
            GoogleContentBlock::Thinking {
                thinking,
                thinking_signature,
            } => {
                thinking.push_str(text);
                *thinking_signature = retain_thought_signature(
                    thinking_signature.as_deref(),
                    part.thought_signature.as_deref(),
                )
                .map(str::to_string);
                let event = GoogleStreamEvent::ThinkingDelta {
                    content_index,
                    delta: text.to_string(),
                };
                self.push_event(event);
            }
            GoogleContentBlock::Text {
                text: current_text,
                text_signature,
            } => {
                current_text.push_str(text);
                *text_signature = retain_thought_signature(
                    text_signature.as_deref(),
                    part.thought_signature.as_deref(),
                )
                .map(str::to_string);
                let event = GoogleStreamEvent::TextDelta {
                    content_index,
                    delta: text.to_string(),
                };
                self.push_event(event);
            }
            GoogleContentBlock::ToolCall { .. } => {
                unreachable!("tool call cannot be current text block")
            }
        }
    }

    fn apply_function_call(
        &mut self,
        function_call: &FunctionCall,
        thought_signature: Option<String>,
    ) {
        let existing_ids = self
            .message
            .content
            .iter()
            .filter_map(|block| match block {
                GoogleContentBlock::ToolCall { id, .. } => Some(id.as_str()),
                GoogleContentBlock::Text { .. } | GoogleContentBlock::Thinking { .. } => None,
            })
            .collect::<HashSet<_>>();
        let id = unique_tool_call_id(
            function_call.name.as_str(),
            function_call.id.as_deref(),
            &existing_ids,
            self.id_timestamp_ms,
            &mut self.tool_call_counter,
        );
        let content_index = self.message.content.len();
        let arguments = function_call.args.clone();
        self.message.content.push(GoogleContentBlock::ToolCall {
            id,
            name: function_call.name.clone(),
            arguments: arguments.clone(),
            thought_signature,
        });
        self.push_event(GoogleStreamEvent::ToolcallStart { content_index });
        self.push_event(GoogleStreamEvent::ToolcallDelta {
            content_index,
            delta: arguments.to_string(),
        });
        self.push_event(GoogleStreamEvent::ToolcallEnd { content_index });
    }

    fn close_current_block(&mut self) {
        let Some(kind) = self.current_block.take() else {
            return;
        };
        let content_index = self.message.content.len() - 1;
        let event = match (kind, &self.message.content[content_index]) {
            (CurrentBlockKind::Text, GoogleContentBlock::Text { text, .. }) => {
                Some(GoogleStreamEvent::TextEnd {
                    content_index,
                    content: text.clone(),
                })
            }
            (CurrentBlockKind::Thinking, GoogleContentBlock::Thinking { thinking, .. }) => {
                Some(GoogleStreamEvent::ThinkingEnd {
                    content_index,
                    content: thinking.clone(),
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            self.push_event(event);
        }
    }

    fn has_tool_calls(&self) -> bool {
        self.message
            .content
            .iter()
            .any(|block| matches!(block, GoogleContentBlock::ToolCall { .. }))
    }

    fn push_event(&mut self, event: GoogleStreamEvent) {
        self.events.push(event.clone());
        self.frames.push(GoogleStreamFrame {
            event,
            partial: self.message.clone(),
        });
    }

    /// Removes frames recorded since the previous call.
    pub fn take_frames(&mut self) -> Vec<GoogleStreamFrame> {
        std::mem::take(&mut self.frames)
    }

    /// Closes the active block and returns the final stream plus terminal frames.
    #[must_use]
    pub fn finish_incremental(
        mut self,
    ) -> (GoogleAssistantMessageEventStream, Vec<GoogleStreamFrame>) {
        self.close_current_block();
        self.push_event(GoogleStreamEvent::Done {
            reason: self.message.stop_reason,
        });
        let frames = self.take_frames();
        (
            GoogleAssistantMessageEventStream {
                events: self.events,
                message: self.message,
            },
            frames,
        )
    }

    fn finish(self) -> GoogleAssistantMessageEventStream {
        self.finish_incremental().0
    }
}

fn unique_tool_call_id(
    name: &str,
    provided_id: Option<&str>,
    existing_ids: &HashSet<&str>,
    timestamp_ms: u64,
    counter: &mut u64,
) -> String {
    if let Some(id) = provided_id.filter(|id| !id.is_empty() && !existing_ids.contains(id)) {
        return id.to_string();
    }

    *counter += 1;
    format!("{name}_{timestamp_ms}_{counter}")
}

fn user_part_to_google_part(item: UserContentPart) -> Part {
    match item {
        UserContentPart::Text { text } => Part {
            text: Some(sanitize_surrogates(&text)),
            ..Part::default()
        },
        UserContentPart::Image { data, mime_type } => Part {
            inline_data: Some(InlineData { mime_type, data }),
            ..Part::default()
        },
    }
}

fn is_valid_thought_signature(signature: &str) -> bool {
    if signature.is_empty() || !signature.len().is_multiple_of(4) {
        return false;
    }

    let padding = signature
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    if padding > 2 || padding == signature.len() {
        return false;
    }

    let (body, suffix) = signature.split_at(signature.len() - padding);
    body.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
        && suffix.bytes().all(|byte| byte == b'=')
}

fn resolve_thought_signature(
    is_same_provider_and_model: bool,
    signature: Option<&str>,
) -> Option<&str> {
    signature
        .filter(|signature| is_same_provider_and_model && is_valid_thought_signature(signature))
}

fn get_gemini_major_version(model_id: &str) -> Option<u64> {
    let lower = model_id.to_ascii_lowercase();
    let rest = lower.strip_prefix("gemini")?;
    let version = rest
        .strip_prefix("-live-")
        .or_else(|| rest.strip_prefix('-'))?;
    let digits: String = version
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn supports_multimodal_function_response(model_id: &str) -> bool {
    get_gemini_major_version(model_id).is_none_or(|version| version >= 3)
}

fn sanitize_for_open_api(schema: &Value) -> Value {
    const META_DECLARATIONS: [&str; 8] = [
        "$schema",
        "$id",
        "$anchor",
        "$dynamicAnchor",
        "$vocabulary",
        "$comment",
        "$defs",
        "definitions",
    ];

    match schema {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !META_DECLARATIONS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), sanitize_for_open_api(value)))
                .collect::<Map<_, _>>(),
        ),
        _ => schema.clone(),
    }
}

fn sanitize_surrogates(text: &str) -> String {
    text.to_string()
}

fn transform_messages(messages: &[Message], model: &Model) -> Vec<Message> {
    let mut tool_call_id_map = HashMap::new();
    let image_aware_messages: Vec<_> = if model.input.contains(&ModelInput::Image) {
        messages.to_vec()
    } else {
        messages.iter().map(downgrade_unsupported_images).collect()
    };

    let transformed: Vec<_> = image_aware_messages
        .into_iter()
        .map(|msg| match msg {
            Message::Assistant {
                content,
                api,
                provider,
                model: source_model,
                stop_reason,
            } => {
                let is_same_model =
                    provider == model.provider && api == model.api && source_model == model.id;
                let transformed_content = content
                    .into_iter()
                    .filter_map(|block| match block {
                        AssistantContent::Thinking {
                            thinking,
                            thinking_signature,
                            redacted,
                        } => {
                            if redacted {
                                return is_same_model.then_some(AssistantContent::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                });
                            }
                            if is_same_model && thinking_signature.is_some() {
                                return Some(AssistantContent::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                });
                            }
                            if thinking.trim().is_empty() {
                                return None;
                            }
                            if is_same_model {
                                Some(AssistantContent::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                })
                            } else {
                                Some(AssistantContent::Text {
                                    text: thinking,
                                    text_signature: None,
                                })
                            }
                        }
                        AssistantContent::Text {
                            text,
                            text_signature,
                        } => Some(AssistantContent::Text {
                            text,
                            text_signature: is_same_model.then_some(text_signature).flatten(),
                        }),
                        AssistantContent::ToolCall {
                            id,
                            name,
                            arguments,
                            thought_signature,
                        } => {
                            let normalized_id =
                                if !is_same_model && requires_tool_call_id(&model.id) {
                                    let normalized = normalize_tool_call_id(&id);
                                    if normalized != id {
                                        tool_call_id_map.insert(id, normalized.clone());
                                    }
                                    normalized
                                } else {
                                    id
                                };
                            Some(AssistantContent::ToolCall {
                                id: normalized_id,
                                name,
                                arguments,
                                thought_signature: is_same_model
                                    .then_some(thought_signature)
                                    .flatten(),
                            })
                        }
                    })
                    .collect();
                Message::Assistant {
                    content: transformed_content,
                    api,
                    provider,
                    model: source_model,
                    stop_reason,
                }
            }
            Message::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
            } => Message::ToolResult {
                tool_call_id: tool_call_id_map
                    .get(&tool_call_id)
                    .cloned()
                    .unwrap_or(tool_call_id),
                tool_name,
                content,
                is_error,
            },
            Message::User { content } => Message::User { content },
        })
        .collect();

    insert_synthetic_tool_results(transformed)
}

fn normalize_tool_call_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

fn downgrade_unsupported_images(message: &Message) -> Message {
    match message {
        Message::User {
            content: UserContent::Parts(content),
        } => Message::User {
            content: UserContent::Parts(replace_images_with_placeholder(
                content,
                "(image omitted: model does not support images)",
            )),
        },
        Message::ToolResult {
            tool_call_id,
            tool_name,
            content,
            is_error,
        } => Message::ToolResult {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            content: replace_images_with_placeholder(
                content,
                "(tool image omitted: model does not support images)",
            ),
            is_error: *is_error,
        },
        _ => message.clone(),
    }
}

fn replace_images_with_placeholder(
    content: &[UserContentPart],
    placeholder: &str,
) -> Vec<UserContentPart> {
    let mut result = Vec::new();
    let mut previous_was_placeholder = false;

    for block in content {
        match block {
            UserContentPart::Image { .. } => {
                if !previous_was_placeholder {
                    result.push(UserContentPart::Text {
                        text: placeholder.to_string(),
                    });
                }
                previous_was_placeholder = true;
            }
            UserContentPart::Text { text } => {
                result.push(block.clone());
                previous_was_placeholder = text == placeholder;
            }
        }
    }

    result
}

fn insert_synthetic_tool_results(transformed: Vec<Message>) -> Vec<Message> {
    let mut result = Vec::new();
    let mut pending_tool_calls: Vec<(String, String)> = Vec::new();
    let mut existing_tool_result_ids = HashSet::new();

    for msg in transformed {
        match msg {
            Message::Assistant {
                content,
                api,
                provider,
                model,
                stop_reason,
            } => {
                insert_missing_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                if matches!(stop_reason, StopReason::Error | StopReason::Aborted) {
                    continue;
                }
                pending_tool_calls = content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContent::ToolCall { id, name, .. } => {
                            Some((id.clone(), name.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                if !pending_tool_calls.is_empty() {
                    existing_tool_result_ids.clear();
                }
                result.push(Message::Assistant {
                    content,
                    api,
                    provider,
                    model,
                    stop_reason,
                });
            }
            Message::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
            } => {
                existing_tool_result_ids.insert(tool_call_id.clone());
                result.push(Message::ToolResult {
                    tool_call_id,
                    tool_name,
                    content,
                    is_error,
                });
            }
            Message::User { content } => {
                insert_missing_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                result.push(Message::User { content });
            }
        }
    }

    insert_missing_tool_results(
        &mut result,
        &mut pending_tool_calls,
        &mut existing_tool_result_ids,
    );
    result
}

fn insert_missing_tool_results(
    result: &mut Vec<Message>,
    pending_tool_calls: &mut Vec<(String, String)>,
    existing_tool_result_ids: &mut HashSet<String>,
) {
    for (id, name) in pending_tool_calls.drain(..) {
        if !existing_tool_result_ids.contains(&id) {
            result.push(Message::ToolResult {
                tool_call_id: id,
                tool_name: name,
                content: vec![UserContentPart::Text {
                    text: "No result provided".to_string(),
                }],
                is_error: true,
            });
        }
    }
    existing_tool_result_ids.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str) -> Model {
        Model {
            id: id.to_string(),
            api: "google-generative-ai".to_string(),
            provider: "google".to_string(),
            input: vec![ModelInput::Text, ModelInput::Image],
        }
    }

    fn gemini3_model(api: &str, provider: &str, id: &str) -> Model {
        Model {
            id: id.to_string(),
            api: api.to_string(),
            provider: provider.to_string(),
            input: vec![ModelInput::Text],
        }
    }

    fn context_with_tool_calls(model: &Model, thought_signature: Option<&str>) -> Context {
        Context {
            messages: vec![
                Message::User {
                    content: UserContent::Text("Hi".to_string()),
                },
                Message::Assistant {
                    content: vec![
                        AssistantContent::ToolCall {
                            id: "call_1".to_string(),
                            name: "bash".to_string(),
                            arguments: Some(json!({ "command": "echo hi" })),
                            thought_signature: thought_signature.map(str::to_string),
                        },
                        AssistantContent::ToolCall {
                            id: "call_2".to_string(),
                            name: "bash".to_string(),
                            arguments: Some(json!({ "command": "ls -la" })),
                            thought_signature: None,
                        },
                    ],
                    api: model.api.clone(),
                    provider: model.provider.clone(),
                    model: model.id.clone(),
                    stop_reason: StopReason::ToolUse,
                },
            ],
        }
    }

    fn make_tool(parameters: Value) -> Tool {
        Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters,
        }
    }

    #[test]
    fn treats_part_thought_true_as_thinking() {
        assert!(is_thinking_part(&Part {
            thought: Some(true),
            thought_signature: None,
            ..Part::default()
        }));
        assert!(is_thinking_part(&Part {
            thought: Some(true),
            thought_signature: Some("opaque-signature".to_string()),
            ..Part::default()
        }));
    }

    #[test]
    fn does_not_treat_thought_signature_alone_as_thinking() {
        // Per Google docs, thoughtSignature is for context replay and can appear on any part type.
        // Only thought === true indicates thinking content.
        // See: https://ai.google.dev/gemini-api/docs/thought-signatures
        assert!(!is_thinking_part(&Part {
            thought: None,
            thought_signature: Some("opaque-signature".to_string()),
            ..Part::default()
        }));
        assert!(!is_thinking_part(&Part {
            thought: Some(false),
            thought_signature: Some("opaque-signature".to_string()),
            ..Part::default()
        }));
    }

    #[test]
    fn does_not_treat_empty_or_missing_signatures_as_thinking_if_thought_is_not_set() {
        assert!(!is_thinking_part(&Part {
            thought: None,
            thought_signature: None,
            ..Part::default()
        }));
        assert!(!is_thinking_part(&Part {
            thought: Some(false),
            thought_signature: Some(String::new()),
            ..Part::default()
        }));
    }

    #[test]
    fn preserves_existing_signature_when_subsequent_deltas_omit_thought_signature() {
        let first = retain_thought_signature(None, Some("sig-1"));
        assert_eq!(first, Some("sig-1"));

        let second = retain_thought_signature(first, None);
        assert_eq!(second, Some("sig-1"));

        let third = retain_thought_signature(second, Some(""));
        assert_eq!(third, Some("sig-1"));
    }

    #[test]
    fn updates_signature_when_new_non_empty_signature_arrives() {
        let updated = retain_thought_signature(Some("sig-1"), Some("sig-2"));
        assert_eq!(updated, Some("sig-2"));
    }

    #[test]
    fn maps_tool_choice_and_stop_reasons() {
        assert_eq!(map_tool_choice("none"), FunctionCallingConfigMode::None);
        assert_eq!(map_tool_choice("missing"), FunctionCallingConfigMode::Auto);
        assert_eq!(map_stop_reason(FinishReason::Stop), StopReason::Stop);
        assert_eq!(map_stop_reason(FinishReason::MaxTokens), StopReason::Length);
        assert_eq!(map_stop_reason_string("OTHER"), StopReason::Error);
    }

    #[test]
    fn google_shared_convert_tools_strips_json_schema_meta_keys_when_use_parameters_true() {
        let tools = [make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$id": "urn:bash-tool",
            "$comment": "A bash tool for demonstration",
            "$defs": {
                "commandDef": { "type": "string" },
            },
            "definitions": {
                "legacyDef": { "type": "number" },
            },
            "type": "object",
            "properties": {
                "command": { "type": "string" },
            },
            "required": ["command"],
        }))];

        let result = convert_tools(&tools, true).expect("tools should convert");
        let decl = &result[0].function_declarations[0];

        assert_eq!(
            decl.parameters,
            Some(json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                },
                "required": ["command"],
            }))
        );
        let parameters = decl.parameters.as_ref().expect("parameters should exist");
        assert!(parameters.get("$schema").is_none());
        assert!(parameters.get("$id").is_none());
        assert!(parameters.get("$comment").is_none());
        assert!(parameters.get("$defs").is_none());
        assert!(parameters.get("definitions").is_none());
    }

    #[test]
    fn google_shared_convert_tools_recursively_strips_nested_json_schema_meta_keys() {
        let tools = [make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "deep": {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "$id": "urn:nested",
                    "type": "string",
                },
            },
        }))];

        let result = convert_tools(&tools, true).expect("tools should convert");
        let decl = &result[0].function_declarations[0];

        assert_eq!(
            decl.parameters,
            Some(json!({
                "type": "object",
                "properties": {
                    "deep": {
                        "type": "string",
                    },
                },
            }))
        );
    }

    #[test]
    fn google_shared_convert_tools_preserves_ref_while_stripping_meta_keys() {
        let tools = [make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "refProp": {
                    "$ref": "#/$defs/someDef",
                    "type": "string",
                },
            },
        }))];

        let result = convert_tools(&tools, true).expect("tools should convert");
        let decl = &result[0].function_declarations[0];

        assert_eq!(
            decl.parameters,
            Some(json!({
                "type": "object",
                "properties": {
                    "refProp": {
                        "$ref": "#/$defs/someDef",
                        "type": "string",
                    },
                },
            }))
        );
    }

    #[test]
    fn google_shared_convert_tools_does_not_mutate_original_parameters_object() {
        let original_parameters = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "command": { "type": "string" },
            },
            "required": ["command"],
        });
        let tools = [make_tool(original_parameters.clone())];

        let _ = convert_tools(&tools, true);

        assert_eq!(
            original_parameters,
            json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                },
                "required": ["command"],
            })
        );
        assert_eq!(tools[0].parameters, original_parameters);
    }

    #[test]
    fn google_shared_convert_tools_preserves_schema_in_parameters_json_schema_when_use_parameters_false()
     {
        let tools = [make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "command": { "type": "string" },
            },
            "required": ["command"],
        }))];

        let result = convert_tools(&tools, false).expect("tools should convert");
        let decl = &result[0].function_declarations[0];

        assert_eq!(
            decl.parameters_json_schema,
            Some(json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                },
                "required": ["command"],
            }))
        );
    }

    #[test]
    fn google_shared_convert_tools_handles_tools_without_schema_gracefully() {
        let tools = [make_tool(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
            },
            "required": ["path"],
        }))];

        let result = convert_tools(&tools, true).expect("tools should convert");
        let decl = &result[0].function_declarations[0];

        assert_eq!(
            decl.parameters,
            Some(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                },
                "required": ["path"],
            }))
        );
    }

    #[test]
    fn google_shared_convert_tools_returns_none_for_empty_tool_list() {
        assert!(convert_tools(&[], false).is_none());
        assert!(convert_tools(&[], true).is_none());
    }

    #[test]
    fn convert_messages_does_not_add_skip_validator_for_unsigned_google_gen_ai_tool_calls() {
        let model = gemini3_model("google-generative-ai", "google", "gemini-3-pro-preview");
        let source_model = gemini3_model("google-generative-ai", "google", "other-model");
        let contents = convert_messages(&model, &context_with_tool_calls(&source_model, None));

        let model_turn = contents
            .iter()
            .find(|content| content.role == "model")
            .expect("model turn exists");
        let function_call_parts = model_turn
            .parts
            .iter()
            .filter(|part| part.function_call.is_some())
            .collect::<Vec<_>>();
        assert_eq!(function_call_parts.len(), 2);
        assert!(function_call_parts[0].thought_signature.is_none());
        assert!(function_call_parts[1].thought_signature.is_none());
        assert!(
            !serde_json::to_string(model_turn)
                .expect("model turn serializes")
                .contains("skip_thought_signature_validator")
        );

        let historical_text = model_turn
            .parts
            .iter()
            .filter_map(|part| part.text.as_deref())
            .filter(|text| text.contains("Historical context"))
            .count();
        assert_eq!(historical_text, 0);
    }

    #[test]
    fn convert_messages_does_not_add_skip_validator_for_unsigned_vertex_tool_calls() {
        let model = gemini3_model("google-vertex", "google-vertex", "gemini-3-pro-preview");
        let contents = convert_messages(&model, &context_with_tool_calls(&model, None));
        let model_turn = contents
            .iter()
            .find(|content| content.role == "model")
            .expect("model turn exists");
        let function_call_parts = model_turn
            .parts
            .iter()
            .filter(|part| part.function_call.is_some())
            .collect::<Vec<_>>();

        assert_eq!(function_call_parts.len(), 2);
        assert!(function_call_parts[0].thought_signature.is_none());
        assert!(function_call_parts[1].thought_signature.is_none());
        assert!(
            !serde_json::to_string(model_turn)
                .expect("model turn serializes")
                .contains("skip_thought_signature_validator")
        );
    }

    #[test]
    fn convert_messages_preserves_valid_tool_call_thought_signature_for_same_provider_and_model() {
        let model = gemini3_model("google-generative-ai", "google", "gemini-3-pro-preview");
        let valid_signature = "AAAAAAAAAAAAAAAAAAAAAA==";
        let contents = convert_messages(
            &model,
            &context_with_tool_calls(&model, Some(valid_signature)),
        );
        let model_turn = contents
            .iter()
            .find(|content| content.role == "model")
            .expect("model turn exists");
        let function_call_parts = model_turn
            .parts
            .iter()
            .filter(|part| part.function_call.is_some())
            .collect::<Vec<_>>();

        assert_eq!(function_call_parts.len(), 2);
        assert_eq!(
            function_call_parts[0].thought_signature,
            Some(valid_signature.to_string())
        );
        assert!(function_call_parts[1].thought_signature.is_none());
    }

    #[test]
    fn convert_messages_does_not_add_thought_signature_for_non_gemini3_models() {
        let model = gemini3_model("google-generative-ai", "google", "gemini-2.5-flash");
        let source_model = gemini3_model("google-generative-ai", "google", "other-model");
        let contents = convert_messages(&model, &context_with_tool_calls(&source_model, None));
        let model_turn = contents
            .iter()
            .find(|content| content.role == "model")
            .expect("model turn exists");
        let function_call_part = model_turn
            .parts
            .iter()
            .find(|part| part.function_call.is_some())
            .expect("function call part exists");

        assert!(function_call_part.thought_signature.is_none());
    }

    #[test]
    fn convert_messages_preserves_valid_same_model_thought_signatures_only() {
        let context = Context {
            messages: vec![Message::Assistant {
                api: "google-generative-ai".to_string(),
                provider: "google".to_string(),
                model: "gemini-3-pro".to_string(),
                stop_reason: StopReason::Stop,
                content: vec![AssistantContent::Thinking {
                    thinking: "reasoning".to_string(),
                    thinking_signature: Some("YWJjZA==".to_string()),
                    redacted: false,
                }],
            }],
        };

        let contents = convert_messages(&model("gemini-3-pro"), &context);
        assert_eq!(contents[0].role, "model");
        assert_eq!(contents[0].parts[0].thought, Some(true));
        assert_eq!(
            contents[0].parts[0].thought_signature.as_deref(),
            Some("YWJjZA==")
        );

        let cross_model = convert_messages(&model("gemini-3-flash"), &context);
        assert_eq!(cross_model[0].parts[0].thought, None);
        assert_eq!(cross_model[0].parts[0].thought_signature, None);
    }

    #[test]
    fn convert_messages_adds_synthetic_tool_result_for_orphaned_tool_call() {
        let context = Context {
            messages: vec![Message::Assistant {
                api: "google-generative-ai".to_string(),
                provider: "google".to_string(),
                model: "claude-test".to_string(),
                stop_reason: StopReason::Stop,
                content: vec![AssistantContent::ToolCall {
                    id: "bad|id".to_string(),
                    name: "run".to_string(),
                    arguments: None,
                    thought_signature: None,
                }],
            }],
        };

        let contents = convert_messages(&model("claude-test"), &context);
        assert_eq!(contents.len(), 2);
        assert_eq!(
            contents[0].parts[0]
                .function_call
                .as_ref()
                .unwrap()
                .id
                .as_deref(),
            Some("bad|id")
        );
        assert_eq!(
            contents[1].parts[0]
                .function_response
                .as_ref()
                .unwrap()
                .response,
            json!({"error":"No result provided"})
        );
    }

    fn tool_result_routing_context(model_id: &str) -> Context {
        Context {
            messages: vec![
                Message::User {
                    content: UserContent::Text("read the files".to_string()),
                },
                Message::Assistant {
                    api: "google-generative-ai".to_string(),
                    provider: "google".to_string(),
                    model: model_id.to_string(),
                    stop_reason: StopReason::ToolUse,
                    content: vec![
                        AssistantContent::ToolCall {
                            id: "call_a".to_string(),
                            name: "read".to_string(),
                            arguments: Some(json!({ "path": "a.txt" })),
                            thought_signature: None,
                        },
                        AssistantContent::ToolCall {
                            id: "call_img".to_string(),
                            name: "read".to_string(),
                            arguments: Some(json!({ "path": "image.png" })),
                            thought_signature: None,
                        },
                        AssistantContent::ToolCall {
                            id: "call_b".to_string(),
                            name: "read".to_string(),
                            arguments: Some(json!({ "path": "b.txt" })),
                            thought_signature: None,
                        },
                    ],
                },
                Message::ToolResult {
                    tool_call_id: "call_a".to_string(),
                    tool_name: "read".to_string(),
                    content: vec![UserContentPart::Text {
                        text: "alpha text".to_string(),
                    }],
                    is_error: false,
                },
                Message::ToolResult {
                    tool_call_id: "call_img".to_string(),
                    tool_name: "read".to_string(),
                    content: vec![UserContentPart::Image {
                        data: "abc".to_string(),
                        mime_type: "image/png".to_string(),
                    }],
                    is_error: false,
                },
                Message::ToolResult {
                    tool_call_id: "call_b".to_string(),
                    tool_name: "read".to_string(),
                    content: vec![UserContentPart::Text {
                        text: "beta text".to_string(),
                    }],
                    is_error: false,
                },
            ],
        }
    }

    #[test]
    fn keeps_separate_synthetic_image_turn_for_gemini_2_google_api_models() {
        let model = model("gemini-2.5-flash");
        let contents = convert_messages(&model, &tool_result_routing_context(&model.id));

        assert_eq!(contents.len(), 5);
        assert!(
            contents[2]
                .parts
                .iter()
                .all(|part| part.function_response.is_some())
        );
        assert_eq!(
            contents[3].parts[0].text.as_deref(),
            Some("Tool result image:")
        );
        assert!(contents[3].parts[1].inline_data.is_some());
        assert!(contents[4].parts[0].function_response.is_some());
    }

    #[test]
    fn nests_image_tool_results_for_gemini_3_google_api_models() {
        let model = model("gemini-3-pro-preview");
        let contents = convert_messages(&model, &tool_result_routing_context(&model.id));

        assert_eq!(contents.len(), 3);
        let tool_result_turn = &contents[2];
        assert_eq!(tool_result_turn.parts.len(), 3);
        let image_response = tool_result_turn.parts[1]
            .function_response
            .as_ref()
            .expect("image tool result should be a function response");
        let image_parts = image_response
            .parts
            .as_ref()
            .expect("image response should carry multimodal parts");
        assert_eq!(image_parts.len(), 1);
        assert!(image_parts[0].inline_data.is_some());
    }
}
