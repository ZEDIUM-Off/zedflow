use std::path::Path;

use crate::config::get_docs_path;

const UNKNOWN_PROVIDER: &str = "unknown";

pub fn get_provider_login_help() -> String {
    get_provider_login_help_for(&get_docs_path())
}

pub fn format_no_models_available_message() -> String {
    format!("No models available. {}", get_provider_login_help())
}

pub fn format_no_model_selected_message() -> String {
    format!(
        "No model selected.\n\n{}\n\nThen use /model to select a model.",
        get_provider_login_help()
    )
}

pub fn format_no_api_key_found_message(provider: &str) -> String {
    let provider = if provider == UNKNOWN_PROVIDER {
        "the selected model"
    } else {
        provider
    };
    format!(
        "No API key found for {provider}.\n\n{}",
        get_provider_login_help()
    )
}

fn get_provider_login_help_for(docs_path: &Path) -> String {
    format!(
        "Use /login to log into a provider via OAuth or API key. See:\n  {}\n  {}",
        docs_path.join("providers.md").display(),
        docs_path.join("models.md").display()
    )
}
