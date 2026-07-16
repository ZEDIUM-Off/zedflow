use std::collections::HashMap;

use zedflow_ai::api::google_vertex::{Context, GoogleVertexOptions, Model, stream};

fn model(base_url: Option<&str>) -> Model {
    Model {
        id: "gemini-3-flash-preview".to_owned(),
        provider: "google-vertex".to_owned(),
        base_url: base_url.map(str::to_owned),
        reasoning: true,
        headers: HashMap::new(),
    }
}

fn adc_options(api_key: Option<&str>) -> GoogleVertexOptions {
    GoogleVertexOptions {
        api_key: api_key.map(str::to_owned),
        project: Some("test-project".to_owned()),
        location: Some("us-central1".to_owned()),
        ..GoogleVertexOptions::default()
    }
}

fn request(
    base_url: Option<&str>,
    options: GoogleVertexOptions,
) -> zedflow_ai::api::google_vertex::PreparedGoogleVertexRequest {
    stream(&model(base_url), &Context::default(), Some(&options))
        .expect("Vertex request")
        .request
}

#[test]
fn placeholder_option_uses_adc() {
    let request = request(None, adc_options(Some("<authenticated>")));
    assert!(request.url.starts_with("https://us-central1-aiplatform.googleapis.com/v1/projects/test-project/locations/us-central1/"));
    assert!(!request.headers.contains_key("x-goog-api-key"));
}

#[test]
fn credentials_marker_uses_adc() {
    let request = request(None, adc_options(Some("gcp-vertex-credentials")));
    assert!(
        request
            .url
            .contains("/projects/test-project/locations/us-central1/")
    );
    assert!(!request.headers.contains_key("x-goog-api-key"));
}

#[test]
fn placeholder_environment_key_uses_adc() {
    let mut options = adc_options(None);
    options.env.insert(
        "GOOGLE_CLOUD_API_KEY".to_owned(),
        "<authenticated>".to_owned(),
    );
    let request = request(None, options);
    assert!(
        request
            .url
            .contains("/projects/test-project/locations/us-central1/")
    );
    assert!(!request.headers.contains_key("x-goog-api-key"));
}

#[test]
fn real_api_key_uses_express_mode_endpoint_and_header() {
    let request = request(
        None,
        GoogleVertexOptions {
            api_key: Some("AIzaSyExampleRealisticLookingApiKey123456".to_owned()),
            ..GoogleVertexOptions::default()
        },
    );
    assert_eq!(
        request.url,
        "https://aiplatform.googleapis.com/v1/publishers/google/models/gemini-3-flash-preview:streamGenerateContent?alt=sse"
    );
    assert_eq!(
        request.headers.get("x-goog-api-key").map(String::as_str),
        Some("AIzaSyExampleRealisticLookingApiKey123456")
    );
}

#[test]
fn generated_vertex_base_url_placeholder_is_not_forwarded() {
    let request = request(
        Some(
            "https://{location}-aiplatform.googleapis.com/v1/projects/test-project/locations/{location}",
        ),
        adc_options(Some("gcp-vertex-credentials")),
    );
    assert!(
        request
            .url
            .starts_with("https://us-central1-aiplatform.googleapis.com/")
    );
    assert!(!request.url.contains("{location}"));
}

#[test]
fn custom_base_url_is_forwarded_for_adc() {
    let request = request(
        Some("https://proxy.example.com"),
        adc_options(Some("gcp-vertex-credentials")),
    );
    assert!(request.url.starts_with("https://proxy.example.com/"));
    assert!(!request.headers.contains_key("x-goog-api-key"));
}

#[test]
fn custom_base_url_is_forwarded_for_api_key() {
    let request = request(
        Some("https://proxy.example.com"),
        GoogleVertexOptions {
            api_key: Some("real-key".to_owned()),
            ..GoogleVertexOptions::default()
        },
    );
    assert!(request.url.starts_with("https://proxy.example.com/"));
    assert_eq!(
        request.headers.get("x-goog-api-key").map(String::as_str),
        Some("real-key")
    );
}

#[test]
fn versioned_custom_base_url_is_not_versioned_again() {
    let request = request(
        Some("https://proxy.example.com/v1/projects/test-project/locations/global"),
        adc_options(Some("gcp-vertex-credentials")),
    );
    assert!(!request.url.contains("/v1/v1/"));
    assert_eq!(request.url.matches("/v1/").count(), 1);
}
