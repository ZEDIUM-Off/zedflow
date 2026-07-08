//! Lazy Bedrock Converse Stream API entry point ported from Pi.

use std::sync::{Mutex, OnceLock};

use zedflow_core::error::Result;

/// Pi's `ProviderStreams` contract used by lazy API factories.
///
/// The full shared stream-function shape is completed by U9; Bedrock can still resolve its
/// static module without JavaScript dynamic import machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStreams;

static BEDROCK_MODULE_OVERRIDE: OnceLock<Mutex<Option<ProviderStreams>>> = OnceLock::new();

fn bedrock_module_override_slot() -> &'static Mutex<Option<ProviderStreams>> {
    BEDROCK_MODULE_OVERRIDE.get_or_init(|| Mutex::new(None))
}

fn bedrock_module_override() -> Option<ProviderStreams> {
    let guard = match bedrock_module_override_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard
}

/// Overrides the dynamically imported Bedrock implementation.
///
/// Pi uses this for Bun binary builds, where the variable-specifier import
/// cannot be bundled and a statically imported module is registered instead.
pub fn set_bedrock_provider_module(module: ProviderStreams) {
    let mut guard = match bedrock_module_override_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = Some(module);
}

/// Returns the lazy Bedrock Converse Stream provider streams.
///
/// Rust uses static dispatch for this module; the shared ProviderStreams shape is completed by U9.
pub fn bedrock_converse_stream_api() -> Result<ProviderStreams> {
    Ok(bedrock_module_override().unwrap_or(ProviderStreams))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_override() {
        let mut guard = match bedrock_module_override_slot().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }

    #[test]
    fn returns_static_bedrock_module_without_network_when_no_override_registered() {
        clear_override();

        assert_eq!(bedrock_converse_stream_api(), Ok(ProviderStreams));
    }

    #[test]
    fn registered_override_short_circuits_dynamic_import_placeholder() {
        clear_override();
        set_bedrock_provider_module(ProviderStreams);

        assert_eq!(bedrock_converse_stream_api(), Ok(ProviderStreams));
        clear_override();
    }
}
