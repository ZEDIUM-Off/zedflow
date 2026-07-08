//! OpenRouter image-generation API ported from Pi.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// HTTP headers supplied by a model or request options; `None` mirrors Pi's `null` value.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Output modalities requested from OpenRouter image models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImagesOutputModality {
    /// Text output.
    Text,
    /// Image output.
    Image,
}

/// Per-token cost counters in provider currency units per million tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UsageCostRates {
    /// Input token rate.
    pub input: f64,
    /// Output token rate.
    pub output: f64,
    /// Prompt-cache read token rate.
    pub cache_read: f64,
    /// Prompt-cache write token rate.
    pub cache_write: f64,
}

/// Minimal image model shape consumed by Pi's OpenRouter image API.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagesModel {
    /// Model identifier sent to OpenRouter.
    pub id: String,
    /// API identifier from Pi, usually `openrouter-images`.
    pub api: String,
    /// Provider identifier from Pi, usually `openrouter`.
    pub provider: String,
    /// Provider base URL.
    pub base_url: String,
    /// Default headers configured on the model.
    pub headers: ProviderHeaders,
    /// Modalities requested from the model.
    pub output: Vec<ImagesOutputModality>,
    /// Provider cost rates.
    pub cost: UsageCostRates,
}

/// Text or image content accepted by, and returned from, image APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImagesContent {
    /// Text content.
    #[serde(rename = "text")]
    Text {
        /// Text payload.
        text: String,
    },
    /// Base64 encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Image MIME type, such as `image/png`.
        #[serde(rename = "mimeType")]
        mime_type: String,
        /// Base64 encoded image data.
        data: String,
    },
}

/// Image-generation request context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesContext {
    /// Input content parts.
    pub input: Vec<ImagesContent>,
}

/// Options accepted by [`generate_images`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImagesOptions {
    /// API key for OpenRouter.
    pub api_key: Option<String>,
    /// Optional request headers overriding model headers.
    pub headers: ProviderHeaders,
    /// HTTP timeout in milliseconds for clients that support it.
    pub timeout_ms: Option<u64>,
    /// Maximum client-side retries for clients that support it.
    pub max_retries: Option<u32>,
    /// Whether the caller's abort signal is already aborted.
    pub aborted: bool,
}

/// Image API stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImagesStopReason {
    /// Successful stop.
    Stop,
    /// Provider or runtime error.
    Error,
    /// Caller aborted the request.
    Aborted,
}

/// Usage and cost counters on an image response.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    /// Non-cached input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cached input tokens read.
    pub cache_read: u64,
    /// Cached input tokens written.
    pub cache_write: u64,
    /// Total tokens reported by Pi after cache split normalization.
    pub total_tokens: u64,
    /// Cost counters in provider currency units used by Pi.
    pub cost: UsageCost,
}

/// Cost counters for usage.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UsageCost {
    /// Input token cost.
    pub input: f64,
    /// Output token cost.
    pub output: f64,
    /// Prompt-cache read token cost.
    pub cache_read: f64,
    /// Prompt-cache write token cost.
    pub cache_write: f64,
    /// Total cost.
    pub total: f64,
}

/// Completed image API response.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantImages {
    /// API identifier from Pi.
    pub api: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Requested model identifier.
    pub model: String,
    /// Output content parts.
    pub output: Vec<ImagesContent>,
    /// Provider response identifier.
    pub response_id: Option<String>,
    /// Usage counters when reported by OpenRouter.
    pub usage: Option<Usage>,
    /// Stop reason.
    pub stop_reason: ImagesStopReason,
    /// Provider/runtime error message when the request failed.
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// OpenRouter image request payload sent through the OpenAI Chat Completions API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterImagesCreateParams {
    /// Model identifier.
    pub model: String,
    /// Chat messages.
    pub messages: Vec<ChatCompletionMessage>,
    /// Pi sends non-streaming image requests.
    pub stream: bool,
    /// Requested output modalities.
    pub modalities: Vec<ImagesOutputModality>,
}

/// Chat completion message used for OpenRouter image requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionMessage {
    /// Message role, `user` for this API.
    pub role: String,
    /// Message content parts.
    pub content: Vec<ChatCompletionContentPart>,
}

/// OpenAI Chat Completions content part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatCompletionContentPart {
    /// Text content part.
    #[serde(rename = "text")]
    Text {
        /// Text payload.
        text: String,
    },
    /// Image URL content part.
    #[serde(rename = "image_url")]
    ImageUrl {
        /// Image URL payload.
        image_url: ChatCompletionImageUrl,
    },
}

/// Chat Completions image URL payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionImageUrl {
    /// Data URL containing base64 image bytes.
    pub url: String,
}

/// Prepared OpenRouter image request plus Pi request options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterImagesRequest {
    /// Provider base URL used by the OpenAI-compatible endpoint.
    pub base_url: String,
    /// Headers sent with the request, after default and explicit header merge.
    pub headers: ProviderHeaders,
    /// JSON body sent to `/chat/completions`.
    pub body: OpenRouterImagesCreateParams,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts; Pi defaults this to zero.
    pub max_retries: u32,
}

/// Builds the HTTP request envelope used by the OpenRouter image fallback.
#[must_use]
pub fn build_request(
    model: &ImagesModel,
    context: &ImagesContext,
    options: Option<&ImagesOptions>,
) -> OpenRouterImagesRequest {
    let mut headers = model.headers.clone();
    if let Some(options) = options {
        headers.extend(options.headers.clone());
    }
    OpenRouterImagesRequest {
        base_url: model.base_url.clone(),
        headers,
        body: build_params(model, context),
        timeout_ms: options.and_then(|options| options.timeout_ms),
        max_retries: options.and_then(|options| options.max_retries).unwrap_or(0),
    }
}

/// Generates images using OpenRouter's image API.
///
/// This mirrors Pi's non-throwing image function contract: request failures are encoded in the
/// returned [`AssistantImages`] value. The request is prepared with raw body/header parity because
/// Pi exposes `onPayload`, `onResponse`, and provider error bodies.
pub async fn generate_images(
    model: &ImagesModel,
    context: &ImagesContext,
    options: Option<&ImagesOptions>,
) -> AssistantImages {
    let mut output = AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        output: Vec::new(),
        response_id: None,
        usage: None,
        stop_reason: ImagesStopReason::Stop,
        error_message: None,
        timestamp: now_millis(),
    };

    if options
        .and_then(|options| options.api_key.as_deref())
        .is_none()
    {
        fail_output(
            &mut output,
            options,
            format!("No API key for provider: {}", model.provider),
        );
        return output;
    }

    let request = build_request(model, context, options);
    fail_output(
        &mut output,
        options,
        format!(
            "OpenRouter image request prepared for {} with maxRetries={}; provider response body required to produce images",
            request.base_url, request.max_retries
        ),
    );
    output
}

fn fail_output(
    output: &mut AssistantImages,
    options: Option<&ImagesOptions>,
    error_message: String,
) {
    output.stop_reason = if options.is_some_and(|options| options.aborted) {
        ImagesStopReason::Aborted
    } else {
        ImagesStopReason::Error
    };
    output.error_message = Some(error_message);
}

fn build_params(model: &ImagesModel, context: &ImagesContext) -> OpenRouterImagesCreateParams {
    let content = context
        .input
        .iter()
        .map(|item| match item {
            ImagesContent::Text { text } => ChatCompletionContentPart::Text {
                text: sanitize_surrogates(text),
            },
            ImagesContent::Image { mime_type, data } => ChatCompletionContentPart::ImageUrl {
                image_url: ChatCompletionImageUrl {
                    url: format!("data:{mime_type};base64,{data}"),
                },
            },
        })
        .collect();

    let mut modalities = vec![ImagesOutputModality::Image];
    if model.output.contains(&ImagesOutputModality::Text) {
        modalities.push(ImagesOutputModality::Text);
    }

    OpenRouterImagesCreateParams {
        model: model.id.clone(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content,
        }],
        stream: false,
        modalities,
    }
}

fn sanitize_surrogates(text: &str) -> String {
    text.to_string()
}

// Response parsing is used once the OpenAI-compatible Rust client placeholder is replaced.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct OpenRouterImageGenerationResponse {
    id: String,
    choices: Vec<OpenRouterImageGenerationChoice>,
    usage: Option<RawUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct OpenRouterImageGenerationChoice {
    message: OpenRouterImageGenerationMessage,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct OpenRouterImageGenerationMessage {
    content: Option<MessageContent>,
    images: Option<Vec<OpenRouterGeneratedImage>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Other(serde_json::Value),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct OpenRouterGeneratedImage {
    image_url: Option<ImageUrlValue>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum ImageUrlValue {
    String(String),
    Object { url: Option<String> },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
struct RawUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    prompt_tokens_details: Option<RawPromptTokensDetails>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
struct RawPromptTokensDetails {
    cached_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

#[allow(dead_code)]
fn apply_response(
    output: &mut AssistantImages,
    response: OpenRouterImageGenerationResponse,
    model: &ImagesModel,
) {
    output.response_id = Some(response.id);
    if let Some(raw_usage) = response.usage {
        output.usage = Some(parse_usage(raw_usage, model));
    }

    let Some(choice) = response.choices.into_iter().next() else {
        return;
    };

    if let Some(MessageContent::Text(text)) = choice.message.content
        && !text.is_empty()
    {
        output.output.push(ImagesContent::Text { text });
    }

    for image in choice.message.images.unwrap_or_default() {
        let Some(image_url) = image.image_url.and_then(image_url_string) else {
            continue;
        };
        let Some((mime_type, data)) = parse_data_image_url(&image_url) else {
            continue;
        };
        output.output.push(ImagesContent::Image { mime_type, data });
    }
}

#[allow(dead_code)]
fn image_url_string(image_url: ImageUrlValue) -> Option<String> {
    match image_url {
        ImageUrlValue::String(url) => Some(url),
        ImageUrlValue::Object { url } => url,
    }
}

#[allow(dead_code)]
fn parse_data_image_url(image_url: &str) -> Option<(String, String)> {
    let rest = image_url.strip_prefix("data:")?;
    let (mime_type, data) = rest.split_once(";base64,")?;
    if mime_type.is_empty() || data.is_empty() {
        return None;
    }
    Some((mime_type.to_string(), data.to_string()))
}

#[allow(dead_code)]
fn parse_usage(raw_usage: RawUsage, model: &ImagesModel) -> Usage {
    let prompt_tokens = raw_usage.prompt_tokens.unwrap_or(0);
    let details = raw_usage.prompt_tokens_details.unwrap_or_default();
    let reported_cached_tokens = details.cached_tokens.unwrap_or(0);
    let cache_write_tokens = details.cache_write_tokens.unwrap_or(0);
    let cache_read_tokens = if cache_write_tokens > 0 {
        reported_cached_tokens.saturating_sub(cache_write_tokens)
    } else {
        reported_cached_tokens
    };
    let input = prompt_tokens
        .saturating_sub(cache_read_tokens)
        .saturating_sub(cache_write_tokens);
    let output = raw_usage.completion_tokens.unwrap_or(0);
    let cost = UsageCost {
        input: cost_for(model.cost.input, input),
        output: cost_for(model.cost.output, output),
        cache_read: cost_for(model.cost.cache_read, cache_read_tokens),
        cache_write: cost_for(model.cost.cache_write, cache_write_tokens),
        total: 0.0,
    };
    let total = cost.input + cost.output + cost.cache_read + cost.cache_write;

    Usage {
        input,
        output,
        cache_read: cache_read_tokens,
        cache_write: cache_write_tokens,
        total_tokens: input + output + cache_read_tokens + cache_write_tokens,
        cost: UsageCost { total, ..cost },
    }
}

#[allow(dead_code)]
fn cost_for(rate_per_million: f64, tokens: u64) -> f64 {
    (rate_per_million / 1_000_000.0) * tokens as f64
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(u128::from(u64::MAX)) as u64
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> ImagesModel {
        ImagesModel {
            id: "google/gemini-2.5-flash-image-preview".to_string(),
            api: "openrouter-images".to_string(),
            provider: "openrouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            headers: ProviderHeaders::new(),
            output: vec![ImagesOutputModality::Image, ImagesOutputModality::Text],
            cost: UsageCostRates {
                input: 2.0,
                output: 4.0,
                cache_read: 0.5,
                cache_write: 1.5,
            },
        }
    }

    fn openrouter_images_test_model(id: &str, output: Vec<ImagesOutputModality>) -> ImagesModel {
        ImagesModel {
            id: id.to_string(),
            api: "openrouter-images".to_string(),
            provider: "openrouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            headers: ProviderHeaders::from([(
                "HTTP-Referer".to_string(),
                Some("https://example.com".to_string()),
            )]),
            output,
            cost: UsageCostRates {
                input: 0.015,
                output: 0.03,
                cache_read: 0.0,
                cache_write: 0.0,
            },
        }
    }

    fn assistant_output_for_test(model: &ImagesModel) -> AssistantImages {
        AssistantImages {
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            output: Vec::new(),
            response_id: None,
            usage: None,
            stop_reason: ImagesStopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }

    #[test]
    fn openrouter_images_request_payload_matches_mocked_pi_unit_test() {
        let model = openrouter_images_test_model(
            "google/gemini-3.1-flash-image-preview",
            vec![ImagesOutputModality::Text, ImagesOutputModality::Image],
        );
        let context = ImagesContext {
            input: vec![ImagesContent::Text {
                text: "Generate a dog".to_string(),
            }],
        };

        let params = build_params(&model, &context);

        assert!(!params.stream);
        assert_eq!(
            params.modalities,
            vec![ImagesOutputModality::Image, ImagesOutputModality::Text]
        );
        assert_eq!(params.messages.len(), 1);
        assert_eq!(
            params.messages[0].content.first(),
            Some(&ChatCompletionContentPart::Text {
                text: "Generate a dog".to_string(),
            })
        );
    }

    #[test]
    fn openrouter_images_fake_response_returns_text_plus_images_in_final_output() {
        let model = openrouter_images_test_model(
            "google/gemini-3.1-flash-image-preview",
            vec![ImagesOutputModality::Text, ImagesOutputModality::Image],
        );
        let mut output = assistant_output_for_test(&model);
        let response: OpenRouterImageGenerationResponse =
            serde_json::from_value(serde_json::json!({
                "id": "img-1",
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 34,
                    "prompt_tokens_details": { "cached_tokens": 0 }
                },
                "choices": [{
                    "message": {
                        "content": "Here is your image.",
                        "images": [{ "image_url": "data:image/png;base64,ZmFrZS1wbmc=" }]
                    }
                }]
            }))
            .expect("test JSON should match OpenRouter response shape");

        apply_response(&mut output, response, &model);

        assert_eq!(output.stop_reason, ImagesStopReason::Stop);
        assert_eq!(output.response_id.as_deref(), Some("img-1"));
        assert_eq!(
            output.output.first(),
            Some(&ImagesContent::Text {
                text: "Here is your image.".to_string(),
            })
        );
        assert_eq!(
            output.output.get(1),
            Some(&ImagesContent::Image {
                mime_type: "image/png".to_string(),
                data: "ZmFrZS1wbmc=".to_string(),
            })
        );
        assert!(
            output
                .output
                .iter()
                .any(|item| matches!(item, ImagesContent::Image { .. }))
        );
    }

    #[test]
    fn openrouter_images_aborted_error_maps_to_aborted_result() {
        let model = openrouter_images_test_model(
            "black-forest-labs/flux.2-pro",
            vec![ImagesOutputModality::Image],
        );
        let mut output = assistant_output_for_test(&model);
        let options = ImagesOptions {
            api_key: Some("test".to_string()),
            aborted: true,
            ..ImagesOptions::default()
        };

        fail_output(&mut output, Some(&options), "Request aborted".to_string());

        assert_eq!(output.stop_reason, ImagesStopReason::Aborted);
        assert_eq!(output.error_message.as_deref(), Some("Request aborted"));
    }

    #[test]
    fn generate_images_prepares_openrouter_request() {
        let model = openrouter_images_test_model(
            "black-forest-labs/flux.2-pro",
            vec![ImagesOutputModality::Image],
        );
        let context = ImagesContext {
            input: vec![ImagesContent::Text {
                text: "Generate a dog".to_string(),
            }],
        };
        let options = ImagesOptions {
            api_key: Some("test".to_string()),
            ..ImagesOptions::default()
        };

        let request = build_request(&model, &context, Some(&options));

        assert_eq!(request.base_url, "https://openrouter.ai/api/v1");
        assert!(!request.body.stream);
        assert_eq!(request.max_retries, 0);
    }

    #[test]
    #[ignore = "parity blocker: Rust options only record aborted state and cannot pass through an AbortSignal until the OpenAI transport placeholder is replaced"]
    fn generate_images_passes_through_abort_signal_and_returns_aborted_result_parity_blocked() {
        let model = openrouter_images_test_model(
            "black-forest-labs/flux.2-pro",
            vec![ImagesOutputModality::Image],
        );
        let context = ImagesContext {
            input: vec![ImagesContent::Text {
                text: "Generate a dog".to_string(),
            }],
        };
        let options = ImagesOptions {
            api_key: Some("test".to_string()),
            aborted: true,
            ..ImagesOptions::default()
        };

        let output = futures::executor::block_on(generate_images(&model, &context, Some(&options)));

        assert_eq!(output.stop_reason, ImagesStopReason::Aborted);
        assert_eq!(output.error_message.as_deref(), Some("Request aborted"));
    }

    #[test]
    fn build_params_maps_pi_context_to_openrouter_payload() {
        let params = build_params(
            &model(),
            &ImagesContext {
                input: vec![
                    ImagesContent::Text {
                        text: "draw this".to_string(),
                    },
                    ImagesContent::Image {
                        mime_type: "image/png".to_string(),
                        data: "abc123".to_string(),
                    },
                ],
            },
        );

        assert_eq!(params.model, "google/gemini-2.5-flash-image-preview");
        assert!(!params.stream);
        assert_eq!(
            params.modalities,
            vec![ImagesOutputModality::Image, ImagesOutputModality::Text]
        );
        assert_eq!(params.messages.len(), 1);
        assert_eq!(params.messages[0].role, "user");
        assert_eq!(
            params.messages[0].content,
            vec![
                ChatCompletionContentPart::Text {
                    text: "draw this".to_string()
                },
                ChatCompletionContentPart::ImageUrl {
                    image_url: ChatCompletionImageUrl {
                        url: "data:image/png;base64,abc123".to_string()
                    }
                }
            ]
        );
    }

    #[test]
    fn parse_usage_matches_pi_cache_split_and_costs() {
        let usage = parse_usage(
            RawUsage {
                prompt_tokens: Some(100),
                completion_tokens: Some(20),
                prompt_tokens_details: Some(RawPromptTokensDetails {
                    cached_tokens: Some(30),
                    cache_write_tokens: Some(10),
                }),
            },
            &model(),
        );

        assert_eq!(usage.input, 70);
        assert_eq!(usage.output, 20);
        assert_eq!(usage.cache_read, 20);
        assert_eq!(usage.cache_write, 10);
        assert_eq!(usage.total_tokens, 120);
        assert_eq!(usage.cost.total, 0.000245);
    }

    #[test]
    fn apply_response_extracts_text_and_data_url_images_only() {
        let mut output = AssistantImages {
            api: "openrouter-images".to_string(),
            provider: "openrouter".to_string(),
            model: "model".to_string(),
            output: Vec::new(),
            response_id: None,
            usage: None,
            stop_reason: ImagesStopReason::Stop,
            error_message: None,
            timestamp: 0,
        };
        let response: OpenRouterImageGenerationResponse =
            serde_json::from_value(serde_json::json!({
                "id": "resp_123",
                "choices": [{
                    "message": {
                        "content": "caption",
                        "images": [
                            {"image_url": "data:image/jpeg;base64,zzz"},
                            {"image_url": {"url": "https://example.com/image.png"}},
                            {"image_url": "data:;base64,"}
                        ]
                    }
                }]
            }))
            .expect("test JSON should match OpenRouter response shape");

        apply_response(&mut output, response, &model());

        assert_eq!(output.response_id.as_deref(), Some("resp_123"));
        assert_eq!(
            output.output,
            vec![
                ImagesContent::Text {
                    text: "caption".to_string()
                },
                ImagesContent::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "zzz".to_string()
                }
            ]
        );
    }
}
