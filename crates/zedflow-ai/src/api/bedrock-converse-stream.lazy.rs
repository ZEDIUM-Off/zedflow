//! Static Bedrock Converse Stream API entry point ported from Pi.

use std::sync::{Mutex, OnceLock};

use crate::types::ProviderStreams;

use super::lazy::terminal_error_api;

static BEDROCK_MODULE_OVERRIDE: OnceLock<Mutex<Option<ProviderStreams>>> = OnceLock::new();

fn bedrock_module_override_slot() -> &'static Mutex<Option<ProviderStreams>> {
    BEDROCK_MODULE_OVERRIDE.get_or_init(|| Mutex::new(None))
}

fn bedrock_module_override() -> Option<ProviderStreams> {
    bedrock_module_override_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Overrides the statically registered Bedrock implementation.
pub fn set_bedrock_provider_module(module: ProviderStreams) {
    *bedrock_module_override_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(module);
}

/// Returns the Bedrock Converse Stream provider streams.
#[must_use]
pub fn bedrock_converse_stream_api() -> ProviderStreams {
    bedrock_module_override().unwrap_or_else(|| terminal_error_api("bedrock-converse-stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_override() {
        *bedrock_module_override_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    #[test]
    fn returns_static_bedrock_module_without_network_when_no_override_registered() {
        clear_override();
        let _ = bedrock_converse_stream_api();
    }
}
