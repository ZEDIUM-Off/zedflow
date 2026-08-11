use std::collections::HashMap;

use zedflow_ai::types::{Api, Model, ProviderHeaders};
use zedflow_coding_agent::provider_attribution::merge_provider_attribution_headers;

fn model(provider: &str, base_url: &str) -> Model<Api> {
    Model {
        provider: provider.into(),
        base_url: base_url.into(),
        ..Default::default()
    }
}

#[test]
fn sdk_openrouter_attribution_preserves_defaults_and_caller_precedence() {
    let provider: ProviderHeaders = HashMap::from([
        (
            "HTTP-Referer".into(),
            Some("https://provider.example".into()),
        ),
        (
            "X-OpenRouter-Categories".into(),
            Some("provider-category".into()),
        ),
    ]);
    let request: ProviderHeaders =
        HashMap::from([("X-OpenRouter-Title".into(), Some("request-title".into()))]);

    let headers = merge_provider_attribution_headers(
        &model("custom-openrouter", "not-a-url-openrouter.ai"),
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
        Some("provider-category")
    );
    assert!(
        merge_provider_attribution_headers(
            &model("openrouter", "https://openrouter.ai/api/v1"),
            false,
            None,
            &[],
        )
        .is_none()
    );
}
