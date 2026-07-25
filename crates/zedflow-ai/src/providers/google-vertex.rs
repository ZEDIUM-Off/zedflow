//! Google Vertex AI provider factory ported from Pi's `packages/ai/src/providers/google-vertex.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

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

/// Creates the google-vertex provider from the static Rust model catalog.
pub fn google_vertex_provider() -> Result<Provider> {
    let provider = static_provider(
        GOOGLE_VERTEX_PROVIDER_ID,
        GOOGLE_VERTEX_PROVIDER_NAME,
        crate::providers::google_vertex_models::google_vertex_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = google_vertex_provider().expect("provider");
        assert_eq!(provider.id, GOOGLE_VERTEX_PROVIDER_ID);
        assert_eq!(provider.name, GOOGLE_VERTEX_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
