//! Port of Pi `packages/ai/test/lazy-module-load.test.ts`.
//!
//! Pi observes Node dynamic `import()` module loading with `registerHooks`. Rust has no
//! equivalent runtime module-resolution hook in this crate yet, and provider SDK crates are not
//! selected. Keep these parity tests ignored rather than faking an empty loaded-module list.

use zedflow_ai::{api::anthropic_messages_lazy, compat, index, providers::all};

const BLOCKER: &str = "PORT PLACEHOLDER: requires Rust parity for Node registerHooks/dynamic import observability, selected provider SDK crates, builtin compat catalog/dispatch, and Anthropic lazy provider streams";

const SDK_SPECIFIERS: &[&str] = &[
    "@anthropic-ai/sdk",
    "openai",
    "@google/genai",
    "@mistralai/mistralai",
    "@aws-sdk/client-bedrock-runtime",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeResult {
    loaded_specifiers: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeAction {
    ImportRootBarrel,
    BuildAllBuiltinProviders,
    ImportCompatEntrypoint,
    StreamThroughAnthropicLazyApiWrapper,
    DispatchThroughStreamSimple,
}

fn run_probe(action: ProbeAction) -> Result<ProbeResult, String> {
    match action {
        ProbeAction::ImportRootBarrel => {
            let _ = index::INDEX_ENTRYPOINT;
        }
        ProbeAction::BuildAllBuiltinProviders => {
            let models = all::builtin_models();
            let _ = models.get_models(None);
        }
        ProbeAction::ImportCompatEntrypoint => {
            let _ = compat::get_api_providers();
        }
        ProbeAction::StreamThroughAnthropicLazyApiWrapper => {
            let _ = anthropic_messages_lazy::anthropic_messages_api();
        }
        ProbeAction::DispatchThroughStreamSimple => {
            let _ = compat::get_model("anthropic", "claude-sonnet-4-6");
        }
    }

    Err(format!(
        "{BLOCKER}; action={action:?}; tracked SDK specifiers={}",
        SDK_SPECIFIERS.join(", ")
    ))
}

#[test]
#[ignore = "PORT PLACEHOLDER: no Rust equivalent for Node registerHooks/dynamic import SDK-load probe yet"]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_importing_root_barrel() {
    let result = run_probe(ProbeAction::ImportRootBarrel)
        .expect("root barrel import should be observable without loading provider SDKs");

    assert_eq!(result.loaded_specifiers, Vec::<&str>::new());
}

#[test]
#[ignore = "PORT PLACEHOLDER: builtin provider catalog/lazy SDK-load observability is not ported yet"]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_building_all_builtin_providers() {
    let result = run_probe(ProbeAction::BuildAllBuiltinProviders)
        .expect("builtinModels().getModels() should be observable without loading provider SDKs");

    assert_eq!(result.loaded_specifiers, Vec::<&str>::new());
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat entrypoint lazy SDK-load observability is not ported yet"]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_importing_compat_entrypoint() {
    let result = run_probe(ProbeAction::ImportCompatEntrypoint)
        .expect("compat entrypoint import should be observable without loading provider SDKs");

    assert_eq!(result.loaded_specifiers, Vec::<&str>::new());
}

#[test]
#[ignore = "PORT PLACEHOLDER: Anthropic lazy API wrapper still returns a provider-stream placeholder"]
fn lazy_provider_module_loading_loads_only_anthropic_sdk_when_streaming_through_lazy_api_wrapper() {
    let result = run_probe(ProbeAction::StreamThroughAnthropicLazyApiWrapper)
        .expect("anthropicMessagesApi().streamSimple(...).result() should load only Anthropic SDK");

    assert_eq!(result.loaded_specifiers, vec!["@anthropic-ai/sdk"]);
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat getModel/streamSimple and builtin provider dispatch are not ported yet"]
fn lazy_provider_module_loading_loads_only_anthropic_sdk_when_dispatching_through_stream_simple() {
    let result = run_probe(ProbeAction::DispatchThroughStreamSimple)
        .expect("compat.streamSimple(...).result() should load only Anthropic SDK");

    assert_eq!(result.loaded_specifiers, vec!["@anthropic-ai/sdk"]);
}
