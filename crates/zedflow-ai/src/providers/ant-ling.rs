//! Ant Ling provider ported from Pi's `packages/ai/src/providers/ant-ling.ts`.

use crate::models::Provider;
use crate::providers::ant_ling_models::ant_ling_models;
use crate::providers::static_catalog::static_provider;

/// Ant Ling OpenAI-compatible base URL.
pub const ANT_LING_BASE_URL: &str = "https://api.ant-ling.com/v1";

/// Env var used for Ant Ling API keys.
pub const ANT_LING_API_KEY_ENV: &str = "ANT_LING_API_KEY";

/// Creates the Ant Ling provider.
#[must_use]
pub fn ant_ling_provider() -> Provider {
    static_provider("ant-ling", "Ant Ling", ant_ling_models())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ant_ling_provider() {
        let provider = ant_ling_provider();
        assert_eq!(provider.id, "ant-ling");
        assert_eq!(provider.get_models().len(), 3);
    }
}
