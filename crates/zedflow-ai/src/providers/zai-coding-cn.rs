//! Z.AI Coding CN provider factory ported from Pi's `packages/ai/src/providers/zai-coding-cn.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Z.AI Coding CN provider id used by Pi.
pub const ZAI_CODING_CN_PROVIDER_ID: &str = "zai-coding-cn";

/// Z.AI Coding CN display name used by Pi.
pub const ZAI_CODING_CN_PROVIDER_NAME: &str = "Z.AI Coding CN";

/// Z.AI Coding CN OpenAI-compatible API base URL used by Pi.
pub const ZAI_CODING_CN_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";

/// Z.AI Coding CN stream API id used by Pi models.
pub const ZAI_CODING_CN_API: &str = "openai-completions";

/// Z.AI Coding CN API-key auth prompt label used by Pi.
pub const ZAI_CODING_CN_API_KEY_AUTH_NAME: &str = "Z.AI Coding CN API key";

/// Environment variables checked for Z.AI Coding CN API-key auth, in Pi precedence order.
pub const ZAI_CODING_CN_API_KEY_ENV_VARS: &[&str] = &["ZAI_CODING_CN_API_KEY"];

/// Creates the zai-coding-cn provider from the static Rust model catalog.
pub fn zai_coding_cn_provider() -> Result<Provider> {
    let provider = static_provider(
        ZAI_CODING_CN_PROVIDER_ID,
        ZAI_CODING_CN_PROVIDER_NAME,
        crate::providers::zai_coding_cn_models::zai_coding_cn_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = zai_coding_cn_provider().expect("provider");
        assert_eq!(provider.id, ZAI_CODING_CN_PROVIDER_ID);
        assert_eq!(provider.name, ZAI_CODING_CN_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
