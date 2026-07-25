//! Google provider factory ported from Pi's `packages/ai/src/providers/google.ts`.

use std::sync::Arc;

use crate::error::Result;

use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Google provider id used by Pi.
pub const GOOGLE_PROVIDER_ID: &str = "google";

/// Google display name used by Pi.
pub const GOOGLE_PROVIDER_NAME: &str = "Google";

/// Google Generative Language API base URL used by Pi.
pub const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Generative AI stream API id used by Pi models.
pub const GOOGLE_API: &str = "google-generative-ai";

/// Gemini API-key auth prompt label used by Pi.
pub const GOOGLE_API_KEY_AUTH_NAME: &str = "Gemini API key";

/// Environment variables checked for Gemini API-key auth, in Pi precedence order.
pub const GOOGLE_API_KEY_ENV_VARS: &[&str] = &["GEMINI_API_KEY"];

/// Creates the google provider from the static Rust model catalog.
pub fn google_provider() -> Result<Provider> {
    let mut provider = static_provider(
        GOOGLE_PROVIDER_ID,
        GOOGLE_PROVIDER_NAME,
        models_from_catalog(crate::providers::google_models::GOOGLE_MODELS),
    );
    provider.auth.api_key = Some(Arc::new(GoogleApiKeyAuth));
    provider.api =
        ProviderApi::Single(crate::api::google_generative_ai_lazy::google_generative_ai_api());
    Ok(provider)
}

#[derive(Debug)]
struct GoogleApiKeyAuth;

impl crate::auth::types::ApiKeyAuth for GoogleApiKeyAuth {
    fn name(&self) -> &str {
        GOOGLE_API_KEY_AUTH_NAME
    }

    fn resolve<'a>(
        &'a self,
        input: crate::auth::types::ApiKeyResolveInput<'a>,
    ) -> crate::auth::types::AuthFuture<
        'a,
        crate::auth::types::AuthResult<Option<crate::auth::types::ResolvedAuth>>,
    > {
        Box::pin(async move {
            if let Some(key) = input
                .credential
                .and_then(|credential| credential.key.as_deref())
                .filter(|key| !key.is_empty())
            {
                return Ok(Some(crate::auth::types::ResolvedAuth {
                    auth: crate::auth::types::ModelAuth {
                        api_key: Some(key.to_owned()),
                        ..crate::auth::types::ModelAuth::default()
                    },
                    env: input
                        .credential
                        .and_then(|credential| credential.env.clone()),
                    source: Some("stored credential".to_owned()),
                }));
            }
            for name in GOOGLE_API_KEY_ENV_VARS {
                if let Some(key) = input.ctx.env(name).await.filter(|key| !key.is_empty()) {
                    return Ok(Some(crate::auth::types::ResolvedAuth {
                        auth: crate::auth::types::ModelAuth {
                            api_key: Some(key),
                            ..crate::auth::types::ModelAuth::default()
                        },
                        env: None,
                        source: Some((*name).to_owned()),
                    }));
                }
            }
            Ok(None)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = google_provider().expect("provider");
        assert_eq!(provider.id, GOOGLE_PROVIDER_ID);
        assert_eq!(provider.name, GOOGLE_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
