use std::path::PathBuf;

use zedflow_coding_agent::{
    auth_guidance::{
        format_no_api_key_found_message, format_no_model_selected_message,
        format_no_models_available_message, get_provider_login_help,
    },
    config::{
        APP_NAME, APP_TITLE, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, PACKAGE_NAME,
        expand_tilde_path, get_agent_dir, get_auth_path, get_bin_dir,
        get_bundled_interactive_asset_path, get_changelog_path, get_custom_themes_dir,
        get_debug_log_path, get_docs_path, get_examples_path, get_export_template_dir,
        get_interactive_assets_dir, get_models_path, get_package_dir, get_package_json_path,
        get_prompts_dir, get_readme_path, get_sessions_dir, get_settings_path,
        get_share_viewer_url, get_themes_dir, get_tools_dir,
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
    assert_eq!(get_changelog_path(), get_package_dir().join("CHANGELOG.md"));
    assert_eq!(
        get_package_json_path(),
        get_package_dir().join("package.json")
    );
    assert_eq!(
        get_themes_dir(),
        get_package_dir().join("src/modes/interactive/theme")
    );
    assert_eq!(
        get_export_template_dir(),
        get_package_dir().join("src/core/export-html")
    );
    assert_eq!(
        get_bundled_interactive_asset_path("logo.png"),
        get_interactive_assets_dir().join("logo.png")
    );
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
    assert_eq!(get_custom_themes_dir(), expected_agent_dir.join("themes"));
    assert_eq!(get_models_path(), expected_agent_dir.join("models.json"));
    assert_eq!(get_auth_path(), expected_agent_dir.join("auth.json"));
    assert_eq!(
        get_settings_path(),
        expected_agent_dir.join("settings.json")
    );
    assert_eq!(get_tools_dir(), expected_agent_dir.join("tools"));
    assert_eq!(get_bin_dir(), expected_agent_dir.join("bin"));
    assert_eq!(get_prompts_dir(), expected_agent_dir.join("prompts"));
    assert_eq!(get_sessions_dir(), expected_agent_dir.join("sessions"));
    assert_eq!(
        get_debug_log_path(),
        expected_agent_dir.join(format!("{APP_NAME}-debug.log"))
    );
}

#[test]
fn formats_share_viewer_urls_like_pi() {
    assert_eq!(
        get_share_viewer_url("gist/id"),
        "https://pi.dev/session/#gist/id"
    );

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "custom_share_viewer_url_child"])
        .env("PI_SHARE_VIEWER_URL", "https://viewer.example/session/")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore]
fn custom_share_viewer_url_child() {
    assert_eq!(
        get_share_viewer_url("abc123"),
        "https://viewer.example/session/#abc123"
    );
}

#[test]
fn package_directory_overrides_are_normalized() {
    let executable = std::env::current_exe().unwrap();
    let home = std::env::temp_dir().join("zedflow package home");
    let tilde_status = std::process::Command::new(&executable)
        .args(["--ignored", "--exact", "tilde_package_directory_child"])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PI_PACKAGE_DIR", "~/pi package")
        .status()
        .unwrap();
    assert!(tilde_status.success());

    #[cfg(windows)]
    {
        let backslash_tilde_status = std::process::Command::new(&executable)
            .args([
                "--ignored",
                "--exact",
                "backslash_tilde_package_directory_child",
            ])
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PI_PACKAGE_DIR", "~\\pi package")
            .status()
            .unwrap();
        assert!(backslash_tilde_status.success());
    }

    let package_dir = home.join("pi package");
    let file_url = reqwest::Url::from_directory_path(&package_dir).unwrap();
    let file_url_status = std::process::Command::new(executable)
        .args(["--ignored", "--exact", "file_url_package_directory_child"])
        .env("PI_PACKAGE_DIR", file_url.as_str())
        .env("EXPECTED_PACKAGE_DIR", &package_dir)
        .status()
        .unwrap();
    assert!(file_url_status.success());

    let cwd = std::env::temp_dir();
    let relative_status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "relative_package_directory_child"])
        .current_dir(&cwd)
        .env("PI_PACKAGE_DIR", "nested/./package/../package")
        .env("EXPECTED_PACKAGE_DIR", cwd.join("nested/package"))
        .status()
        .unwrap();
    assert!(relative_status.success());
}

#[test]
#[ignore]
fn tilde_package_directory_child() {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").unwrap();
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME").unwrap();
    assert_eq!(get_package_dir(), PathBuf::from(home).join("pi package"));
}

#[cfg(windows)]
#[test]
#[ignore]
fn backslash_tilde_package_directory_child() {
    assert_eq!(
        get_package_dir(),
        PathBuf::from(std::env::var_os("USERPROFILE").unwrap()).join("pi package")
    );
}

#[cfg(windows)]
#[test]
fn windows_tilde_uses_userprofile_instead_of_home() {
    let user_profile = std::env::temp_dir().join("zedflow user profile");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "tilde_package_directory_child"])
        .env("HOME", std::env::temp_dir().join("wrong home"))
        .env("USERPROFILE", &user_profile)
        .env("PI_PACKAGE_DIR", "~/pi package")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore]
fn file_url_package_directory_child() {
    assert_eq!(
        get_package_dir(),
        PathBuf::from(std::env::var_os("EXPECTED_PACKAGE_DIR").unwrap())
    );
}

#[test]
#[ignore]
fn relative_package_directory_child() {
    let package_dir = PathBuf::from(std::env::var_os("EXPECTED_PACKAGE_DIR").unwrap());
    assert_eq!(get_docs_path(), package_dir.join("docs"));
    assert_eq!(get_examples_path(), package_dir.join("examples"));
    assert_eq!(get_readme_path(), package_dir.join("README.md"));
}

#[test]
fn missing_home_environment_uses_os_account_home() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "missing_home_environment_child"])
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore]
fn missing_home_environment_child() {
    #[allow(deprecated)]
    let expected = std::env::home_dir().unwrap();
    assert_eq!(expand_tilde_path("~"), expected);
    assert_eq!(expand_tilde_path("~/pi"), expected.join("pi"));
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
