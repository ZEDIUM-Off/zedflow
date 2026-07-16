//! Mistral provider factory ported from Pi's `packages/ai/src/providers/mistral.ts`.

use std::sync::Arc;

use zedflow_core::error::Result;

use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Mistral provider id used by Pi.
pub const MISTRAL_PROVIDER_ID: &str = "mistral";

/// Mistral display name used by Pi.
pub const MISTRAL_PROVIDER_NAME: &str = "Mistral";

/// Mistral API base URL used by Pi.
pub const MISTRAL_BASE_URL: &str = "https://api.mistral.ai";

/// Mistral API-key auth prompt label used by Pi.
pub const MISTRAL_API_KEY_AUTH_NAME: &str = "Mistral API key";

/// Environment variables checked for Mistral API-key auth, in Pi precedence order.
pub const MISTRAL_API_KEY_ENV_VARS: &[&str] = &["MISTRAL_API_KEY"];

/// Mistral stream API id used by Pi models.
pub const MISTRAL_API: &str = "mistral-conversations";

/// Creates the mistral provider from the static Rust model catalog.
pub fn mistral_provider() -> Result<Provider> {
    let mut provider = static_provider(
        MISTRAL_PROVIDER_ID,
        MISTRAL_PROVIDER_NAME,
        models_from_catalog(crate::providers::mistral_models::MISTRAL_MODELS),
    );
    provider.auth.api_key = Some(Arc::new(MistralApiKeyAuth));
    provider.api =
        ProviderApi::Single(crate::api::mistral_conversations_lazy::mistral_conversations_api());
    Ok(provider)
}

#[derive(Debug)]
struct MistralApiKeyAuth;

impl crate::auth::types::ApiKeyAuth for MistralApiKeyAuth {
    fn name(&self) -> &str {
        MISTRAL_API_KEY_AUTH_NAME
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
            for name in MISTRAL_API_KEY_ENV_VARS {
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
        let provider = mistral_provider().expect("provider");
        assert_eq!(provider.id, MISTRAL_PROVIDER_ID);
        assert_eq!(provider.name, MISTRAL_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
