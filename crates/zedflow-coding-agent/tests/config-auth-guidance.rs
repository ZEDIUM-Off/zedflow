use std::path::PathBuf;

use zedflow_coding_agent::{
    auth_guidance::{
        format_no_api_key_found_message, format_no_model_selected_message,
        format_no_models_available_message, get_provider_login_help,
    },
    config::{
        APP_NAME, APP_TITLE, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, PACKAGE_NAME,
        expand_tilde_path, get_agent_dir, get_docs_path, get_examples_path, get_package_dir,
        get_readme_path,
    },
};

#[test]
fn exposes_package_identity_and_asset_paths() {
    assert_eq!(PACKAGE_NAME, "@earendil-works/pi-coding-agent");
    assert_eq!(APP_NAME, "pi");
    assert_eq!(APP_TITLE, "π");
    assert_eq!(CONFIG_DIR_NAME, ".pi");
    assert_eq!(ENV_AGENT_DIR, "PI_CODING_AGENT_DIR");
    assert_eq!(ENV_SESSION_DIR, "PI_CODING_AGENT_SESSION_DIR");
    assert_eq!(get_docs_path(), get_package_dir().join("docs"));
    assert_eq!(get_examples_path(), get_package_dir().join("examples"));
    assert_eq!(get_readme_path(), get_package_dir().join("README.md"));
    assert_eq!(
        expand_tilde_path("relative/path"),
        PathBuf::from("relative/path")
    );
    let expected_agent_dir = std::env::var(ENV_AGENT_DIR)
        .ok()
        .filter(|path| !path.is_empty())
        .map(|path| expand_tilde_path(&path))
        .unwrap_or_else(|| expand_tilde_path("~").join(CONFIG_DIR_NAME).join("agent"));
    assert_eq!(get_agent_dir(), expected_agent_dir);
}

#[test]
fn empty_directory_overrides_use_pi_defaults() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "empty_directory_overrides_child"])
        .env("PI_PACKAGE_DIR", "")
        .env(ENV_AGENT_DIR, "")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore]
fn empty_directory_overrides_child() {
    assert_eq!(get_package_dir(), PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    assert_eq!(
        get_agent_dir(),
        expand_tilde_path("~").join(CONFIG_DIR_NAME).join("agent")
    );
}

#[test]
fn formats_provider_login_guidance_like_pi() {
    let help = format!(
        "Use /login to log into a provider via OAuth or API key. See:\n  {}\n  {}",
        get_docs_path().join("providers.md").display(),
        get_docs_path().join("models.md").display()
    );
    assert_eq!(get_provider_login_help(), help);
    assert_eq!(
        format_no_models_available_message(),
        format!("No models available. {help}")
    );
    assert_eq!(
        format_no_model_selected_message(),
        format!("No model selected.\n\n{help}\n\nThen use /model to select a model.")
    );
    assert_eq!(
        format_no_api_key_found_message("unknown"),
        format!("No API key found for the selected model.\n\n{help}")
    );
    assert_eq!(
        format_no_api_key_found_message("anthropic"),
        format!("No API key found for anthropic.\n\n{help}")
    );
}
