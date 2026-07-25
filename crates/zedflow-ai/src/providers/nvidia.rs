//! NVIDIA provider factory ported from Pi's `packages/ai/src/providers/nvidia.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// NVIDIA provider id used by Pi.
pub const NVIDIA_PROVIDER_ID: &str = "nvidia";

/// NVIDIA display name used by Pi.
pub const NVIDIA_PROVIDER_NAME: &str = "NVIDIA";

/// NVIDIA OpenAI-compatible API base URL used by Pi.
pub const NVIDIA_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// NVIDIA stream API id used by Pi models.
pub const NVIDIA_API: &str = "openai-completions";

/// NVIDIA API-key auth prompt label used by Pi.
pub const NVIDIA_API_KEY_AUTH_NAME: &str = "NVIDIA API key";

/// Environment variables checked for NVIDIA API-key auth, in Pi precedence order.
pub const NVIDIA_API_KEY_ENV_VARS: &[&str] = &["NVIDIA_API_KEY"];

/// Creates the nvidia provider from the static Rust model catalog.
pub fn nvidia_provider() -> Result<Provider> {
    let provider = static_provider(
        NVIDIA_PROVIDER_ID,
        NVIDIA_PROVIDER_NAME,
        models_from_catalog(crate::providers::nvidia_models::NVIDIA_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = nvidia_provider().expect("provider");
        assert_eq!(provider.id, NVIDIA_PROVIDER_ID);
        assert_eq!(provider.name, NVIDIA_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
