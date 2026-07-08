//! OpenRouter image provider factory ported from Pi's `packages/ai/src/providers/openrouter-images.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::images_models::ImagesProvider;

/// OpenRouter image provider id used by Pi.
pub const OPENROUTER_IMAGES_PROVIDER_ID: &str = "openrouter";

/// OpenRouter image provider display name used by Pi.
pub const OPENROUTER_IMAGES_PROVIDER_NAME: &str = "OpenRouter";

/// OpenRouter Images API id used by Pi image models.
pub const OPENROUTER_IMAGES_API: &str = "openrouter-images";

/// OpenRouter API-key auth prompt label used by Pi.
pub const OPENROUTER_IMAGES_API_KEY_AUTH_NAME: &str = "OpenRouter API key";

/// Environment variables checked for OpenRouter image API-key auth, in Pi precedence order.
pub const OPENROUTER_IMAGES_API_KEY_ENV_VARS: &[&str] = &["OPENROUTER_API_KEY"];

/// Creates Pi's OpenRouter image provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/images-models.ts createImagesProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openrouter-images.lazy.ts openrouterImagesApi, references/pi/packages/ai/src/image-models.generated.ts IMAGE_MODELS.openrouter`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createImagesProvider({ id: "openrouter", name: "OpenRouter", auth: { apiKey: envApiKeyAuth("OpenRouter API key", ["OPENROUTER_API_KEY"]) }, models: Object.values(IMAGE_MODELS.openrouter), api: openrouterImagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared image provider auth/API contract can construct
/// Pi's `createImagesProvider` output from `IMAGE_MODELS.openrouter` in Rust.
pub fn openrouter_images_provider() -> Result<ImagesProvider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/images-models.ts createImagesProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openrouter-images.lazy.ts openrouterImagesApi, references/pi/packages/ai/src/image-models.generated.ts IMAGE_MODELS.openrouter",
        "return createImagesProvider({ id: \"openrouter\", name: \"OpenRouter\", auth: { apiKey: envApiKeyAuth(\"OpenRouter API key\", [\"OPENROUTER_API_KEY\"]) }, models: Object.values(IMAGE_MODELS.openrouter), api: openrouterImagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_openrouter_images_provider_blocker() {
        match openrouter_images_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("createImagesProvider")
                );
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("IMAGE_MODELS.openrouter")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openrouterImagesApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("OPENROUTER_API_KEY")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_openrouter_images_provider_constants() {
        assert_eq!(OPENROUTER_IMAGES_PROVIDER_ID, "openrouter");
        assert_eq!(OPENROUTER_IMAGES_PROVIDER_NAME, "OpenRouter");
        assert_eq!(OPENROUTER_IMAGES_API, "openrouter-images");
        assert_eq!(OPENROUTER_IMAGES_API_KEY_AUTH_NAME, "OpenRouter API key");
        assert_eq!(OPENROUTER_IMAGES_API_KEY_ENV_VARS, &["OPENROUTER_API_KEY"]);
    }
}
