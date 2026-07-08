//! Z.AI Coding CN provider factory ported from Pi's `packages/ai/src/providers/zai-coding-cn.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Z.AI Coding CN provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/zai-coding-cn.models.ts ZAI_CODING_CN_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "zai-coding-cn", name: "Z.AI Coding CN", baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4", auth: { apiKey: envApiKeyAuth("Z.AI Coding CN API key", ["ZAI_CODING_CN_API_KEY"]) }, models: Object.values(ZAI_CODING_CN_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Z.AI Coding CN model catalog are available in Rust.
#[must_use]
pub fn zai_coding_cn_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/zai-coding-cn.models.ts ZAI_CODING_CN_MODELS",
        "return createProvider({ id: \"zai-coding-cn\", name: \"Z.AI Coding CN\", baseUrl: \"https://open.bigmodel.cn/api/coding/paas/v4\", auth: { apiKey: envApiKeyAuth(\"Z.AI Coding CN API key\", [\"ZAI_CODING_CN_API_KEY\"]) }, models: Object.values(ZAI_CODING_CN_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_zai_coding_cn_provider_blocker() {
        match zai_coding_cn_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("ZAI_CODING_CN_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_zai_coding_cn_provider_constants() {
        assert_eq!(ZAI_CODING_CN_PROVIDER_ID, "zai-coding-cn");
        assert_eq!(ZAI_CODING_CN_PROVIDER_NAME, "Z.AI Coding CN");
        assert_eq!(
            ZAI_CODING_CN_BASE_URL,
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
        assert_eq!(ZAI_CODING_CN_API, "openai-completions");
        assert_eq!(ZAI_CODING_CN_API_KEY_AUTH_NAME, "Z.AI Coding CN API key");
        assert_eq!(ZAI_CODING_CN_API_KEY_ENV_VARS, &["ZAI_CODING_CN_API_KEY"]);
    }
}
