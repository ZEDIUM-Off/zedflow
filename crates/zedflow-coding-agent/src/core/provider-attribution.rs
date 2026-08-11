//! Provider attribution headers ported from Pi's `core/provider-attribution.ts`.

use std::collections::HashMap;

use reqwest::Url;
use zedflow_ai::types::{Api, Model, ProviderHeaders};

const OPENROUTER_HOST: &str = "openrouter.ai";
const NVIDIA_NIM_HOST: &str = "integrate.api.nvidia.com";
const CLOUDFLARE_API_HOST: &str = "api.cloudflare.com";
const CLOUDFLARE_AI_GATEWAY_HOST: &str = "gateway.ai.cloudflare.com";
const OPENCODE_HOST: &str = "opencode.ai";

fn matches_host(base_url: &str, expected_host: &str) -> bool {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .as_deref()
        == Some(expected_host)
}

fn is_openrouter_model(model: &Model<Api>) -> bool {
    model.provider == "openrouter" || model.base_url.contains(OPENROUTER_HOST)
}

fn is_nvidia_nim_model(model: &Model<Api>) -> bool {
    model.provider == "nvidia" || matches_host(&model.base_url, NVIDIA_NIM_HOST)
}

fn is_cloudflare_model(model: &Model<Api>) -> bool {
    matches!(
        model.provider.as_str(),
        "cloudflare-workers-ai" | "cloudflare-ai-gateway"
    ) || matches_host(&model.base_url, CLOUDFLARE_API_HOST)
        || matches_host(&model.base_url, CLOUDFLARE_AI_GATEWAY_HOST)
}

fn default_attribution_headers(model: &Model<Api>, telemetry_enabled: bool) -> ProviderHeaders {
    if !telemetry_enabled {
        return HashMap::new();
    }

    let headers = if is_openrouter_model(model) {
        [
            ("HTTP-Referer", "https://pi.dev"),
            ("X-OpenRouter-Title", "pi"),
            ("X-OpenRouter-Categories", "cli-agent"),
        ]
        .as_slice()
    } else if is_nvidia_nim_model(model) {
        [("X-BILLING-INVOKE-ORIGIN", "Pi")].as_slice()
    } else if is_cloudflare_model(model) {
        [("User-Agent", "pi-coding-agent")].as_slice()
    } else {
        return HashMap::new();
    };

    headers
        .iter()
        .map(|(key, value)| ((*key).to_owned(), Some((*value).to_owned())))
        .collect()
}

fn session_headers(model: &Model<Api>, session_id: Option<&str>) -> ProviderHeaders {
    let Some(session_id) = session_id else {
        return HashMap::new();
    };
    if !matches!(model.provider.as_str(), "opencode" | "opencode-go")
        && !matches_host(&model.base_url, OPENCODE_HOST)
    {
        return HashMap::new();
    }

    HashMap::from([
        ("x-opencode-session".to_owned(), Some(session_id.to_owned())),
        ("x-opencode-client".to_owned(), Some("pi".to_owned())),
    ])
}

/// Combines Pi's session, telemetry-gated defaults, and caller headers.
///
/// Header sources are applied in order, so a later caller header wins over every
/// earlier source (including a `None` value that suppresses a default header).
pub fn merge_provider_attribution_headers(
    model: &Model<Api>,
    telemetry_enabled: bool,
    session_id: Option<&str>,
    header_sources: &[Option<&ProviderHeaders>],
) -> Option<ProviderHeaders> {
    let mut headers = session_headers(model, session_id);
    headers.extend(default_attribution_headers(model, telemetry_enabled));
    for source in header_sources.iter().flatten() {
        headers.extend((*source).clone());
    }
    (!headers.is_empty()).then_some(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, base_url: &str) -> Model<Api> {
        Model {
            provider: provider.into(),
            base_url: base_url.into(),
            ..Default::default()
        }
    }

    #[test]
    fn attributes_legacy_openrouter_urls_and_respects_caller_precedence() {
        let provider = HashMap::from([(
            "HTTP-Referer".to_owned(),
            Some("https://provider.example".to_owned()),
        )]);
        let request = HashMap::from([(
            "X-OpenRouter-Title".to_owned(),
            Some("request-title".to_owned()),
        )]);
        let headers = merge_provider_attribution_headers(
            &model("custom", "not-a-url-openrouter.ai"),
            true,
            None,
            &[Some(&provider), Some(&request)],
        )
        .unwrap();

        assert_eq!(
            headers["HTTP-Referer"].as_deref(),
            Some("https://provider.example")
        );
        assert_eq!(
            headers["X-OpenRouter-Title"].as_deref(),
            Some("request-title")
        );
        assert_eq!(
            headers["X-OpenRouter-Categories"].as_deref(),
            Some("cli-agent")
        );
    }

    #[test]
    fn telemetry_only_gates_defaults_not_opencode_session_headers() {
        let headers = merge_provider_attribution_headers(
            &model("opencode", "https://opencode.ai/zen/v1"),
            false,
            Some("session"),
            &[],
        )
        .unwrap();

        assert_eq!(headers["x-opencode-session"].as_deref(), Some("session"));
        assert_eq!(headers["x-opencode-client"].as_deref(), Some("pi"));
        assert!(!headers.contains_key("HTTP-Referer"));
    }

    #[test]
    fn matches_only_exact_nvidia_hosts() {
        let headers = merge_provider_attribution_headers(
            &model("custom", "https://integrate.api.nvidia.com/v1"),
            true,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(headers["X-BILLING-INVOKE-ORIGIN"].as_deref(), Some("Pi"));
        assert!(
            merge_provider_attribution_headers(
                &model("custom", "https://integrate.api.nvidia.com.example/v1"),
                true,
                None,
                &[],
            )
            .is_none()
        );
    }
}
