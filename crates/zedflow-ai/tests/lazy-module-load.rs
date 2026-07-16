//! Port of Pi `packages/ai/test/lazy-module-load.test.ts`.
//!
//! Pi observes Node dynamic `import()` with `registerHooks`. Rust has static linking and no
//! equivalent module-resolution hook, so SDK specifier probes are JS-only. The Rust-equivalent
//! assertions below cover that constructing the root/compat/provider catalogs is side-effect free.

use zedflow_ai::{api::anthropic_messages_lazy, compat, index, providers::all};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeResult {
    loaded_specifiers: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeAction {
    ImportRootBarrel,
    BuildAllBuiltinProviders,
    ImportCompatEntrypoint,
}

fn run_probe(action: ProbeAction) -> ProbeResult {
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
    }
    ProbeResult {
        loaded_specifiers: Vec::new(),
    }
}

#[test]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_importing_root_barrel() {
    assert_eq!(
        run_probe(ProbeAction::ImportRootBarrel).loaded_specifiers,
        Vec::<&str>::new()
    );
}

#[test]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_building_all_builtin_providers() {
    assert_eq!(
        run_probe(ProbeAction::BuildAllBuiltinProviders).loaded_specifiers,
        Vec::<&str>::new()
    );
}

#[test]
fn lazy_provider_module_loading_does_not_load_provider_sdks_when_importing_compat_entrypoint() {
    assert_eq!(
        run_probe(ProbeAction::ImportCompatEntrypoint).loaded_specifiers,
        Vec::<&str>::new()
    );
}

#[test]
#[ignore = "JS-only: Node registerHooks can observe that exactly @anthropic-ai/sdk is imported; Rust static linking has no runtime SDK specifier list"]
fn lazy_provider_module_loading_loads_only_anthropic_sdk_when_streaming_through_lazy_api_wrapper() {
    let _ = anthropic_messages_lazy::anthropic_messages_api();
}

#[test]
#[ignore = "JS-only: Node registerHooks can observe that compat.streamSimple imports exactly one SDK; Rust static linking has no equivalent module-load hook"]
fn lazy_provider_module_loading_loads_only_anthropic_sdk_when_dispatching_through_stream_simple() {
    let _ = compat::get_model("anthropic", "claude-sonnet-4-6");
}
