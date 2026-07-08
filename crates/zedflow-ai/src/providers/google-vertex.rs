//! Google Vertex AI provider factory ported from Pi's `packages/ai/src/providers/google-vertex.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Google Vertex AI provider id used by Pi.
pub const GOOGLE_VERTEX_PROVIDER_ID: &str = "google-vertex";

/// Google Vertex AI display name used by Pi.
pub const GOOGLE_VERTEX_PROVIDER_NAME: &str = "Google Vertex AI";

/// Google Vertex AI stream API id used by Pi models.
pub const GOOGLE_VERTEX_API: &str = "google-vertex";

/// Google Vertex AI API-key auth prompt label used by Pi.
pub const GOOGLE_VERTEX_API_KEY_AUTH_NAME: &str = "Google Cloud credentials";

/// Environment variables checked for explicit Google Vertex AI API-key auth, in Pi precedence order.
pub const GOOGLE_VERTEX_API_KEY_ENV_VARS: &[&str] = &["GOOGLE_CLOUD_API_KEY"];

/// Default Application Default Credentials path checked by Pi.
pub const GOOGLE_VERTEX_ADC_PATH: &str = "~/.config/gcloud/application_default_credentials.json";

/// Application Default Credentials path override used by Pi.
pub const GOOGLE_APPLICATION_CREDENTIALS_ENV: &str = "GOOGLE_APPLICATION_CREDENTIALS";

/// Google Cloud project environment variables accepted by Pi, in precedence order.
pub const GOOGLE_VERTEX_PROJECT_ENV_VARS: &[&str] = &["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"];

/// Google Cloud location environment variable required by Pi for ADC auth.
pub const GOOGLE_CLOUD_LOCATION_ENV: &str = "GOOGLE_CLOUD_LOCATION";

/// Creates Pi's Google Vertex AI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/types.ts ApiKeyAuth, references/pi/packages/ai/src/api/google-vertex.lazy.ts googleVertexApi, references/pi/packages/ai/src/providers/google-vertex.models.ts GOOGLE_VERTEX_MODELS`.
/// Reason: no Rust replacement selected yet for provider auth fields, model catalog wiring, and lazy stream API binding.
/// Required behavior: `return createProvider({ id: "google-vertex", name: "Google Vertex AI", auth: { apiKey: vertexAuth }, models: Object.values(GOOGLE_VERTEX_MODELS), api: googleVertexApi() })`, where `vertexAuth` first accepts a stored key or `GOOGLE_CLOUD_API_KEY`, then accepts Application Default Credentials only when credentials exist and `GOOGLE_CLOUD_PROJECT`/`GCLOUD_PROJECT` plus `GOOGLE_CLOUD_LOCATION` are set.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until Google Vertex provider auth, model catalog, and stream API
/// wiring are available in Rust.
pub fn google_vertex_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/types.ts ApiKeyAuth, references/pi/packages/ai/src/api/google-vertex.lazy.ts googleVertexApi, references/pi/packages/ai/src/providers/google-vertex.models.ts GOOGLE_VERTEX_MODELS",
        "return createProvider({ id: \"google-vertex\", name: \"Google Vertex AI\", auth: { apiKey: vertexAuth }, models: Object.values(GOOGLE_VERTEX_MODELS), api: googleVertexApi() }); vertexAuth uses stored key/GOOGLE_CLOUD_API_KEY or ADC with GOOGLE_APPLICATION_CREDENTIALS/default ADC, GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT, and GOOGLE_CLOUD_LOCATION",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_google_vertex_provider_blocker() {
        let err = google_vertex_provider().expect_err("provider creation is intentionally blocked");
        match err {
            Error::PortPlaceholder(placeholder) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("GOOGLE_VERTEX_MODELS")
                );
                assert!(placeholder.required_behavior().contains("googleVertexApi"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("GOOGLE_CLOUD_LOCATION")
                );
            }
            _ => panic!("unexpected provider error: {err:?}"),
        }
    }

    #[test]
    fn preserves_google_vertex_auth_constants() {
        assert_eq!(GOOGLE_VERTEX_PROVIDER_ID, "google-vertex");
        assert_eq!(GOOGLE_VERTEX_PROVIDER_NAME, "Google Vertex AI");
        assert_eq!(GOOGLE_VERTEX_API, "google-vertex");
        assert_eq!(GOOGLE_VERTEX_API_KEY_AUTH_NAME, "Google Cloud credentials");
        assert_eq!(GOOGLE_VERTEX_API_KEY_ENV_VARS, &["GOOGLE_CLOUD_API_KEY"]);
        assert_eq!(
            GOOGLE_VERTEX_ADC_PATH,
            "~/.config/gcloud/application_default_credentials.json"
        );
        assert_eq!(
            GOOGLE_APPLICATION_CREDENTIALS_ENV,
            "GOOGLE_APPLICATION_CREDENTIALS"
        );
        assert_eq!(
            GOOGLE_VERTEX_PROJECT_ENV_VARS,
            &["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"]
        );
        assert_eq!(GOOGLE_CLOUD_LOCATION_ENV, "GOOGLE_CLOUD_LOCATION");
    }
}
