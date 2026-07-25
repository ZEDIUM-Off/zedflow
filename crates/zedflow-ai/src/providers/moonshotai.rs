//! Moonshot AI provider factory ported from Pi's `packages/ai/src/providers/moonshotai.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Moonshot AI provider id used by Pi.
pub const MOONSHOTAI_PROVIDER_ID: &str = "moonshotai";

/// Moonshot AI display name used by Pi.
pub const MOONSHOTAI_PROVIDER_NAME: &str = "Moonshot AI";

/// Moonshot AI OpenAI-compatible API base URL used by Pi.
pub const MOONSHOTAI_BASE_URL: &str = "https://api.moonshot.ai/v1";

/// Moonshot AI stream API id used by Pi models.
pub const MOONSHOTAI_API: &str = "openai-completions";

/// Moonshot AI API-key auth prompt label used by Pi.
pub const MOONSHOTAI_API_KEY_AUTH_NAME: &str = "Moonshot AI API key";

/// Environment variables checked for Moonshot AI API-key auth, in Pi precedence order.
pub const MOONSHOTAI_API_KEY_ENV_VARS: &[&str] = &["MOONSHOT_API_KEY"];

/// Creates the moonshotai provider from the static Rust model catalog.
pub fn moonshotai_provider() -> Result<Provider> {
    let provider = static_provider(
        MOONSHOTAI_PROVIDER_ID,
        MOONSHOTAI_PROVIDER_NAME,
        crate::providers::moonshotai_models::moonshotai_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = moonshotai_provider().expect("provider");
        assert_eq!(provider.id, MOONSHOTAI_PROVIDER_ID);
        assert_eq!(provider.name, MOONSHOTAI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
