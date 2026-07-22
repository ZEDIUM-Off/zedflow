use std::path::PathBuf;

use zedflow_coding_agent::{
    auth_guidance::{
        format_no_api_key_found_message, format_no_model_selected_message,
        format_no_models_available_message, get_provider_login_help,
    },
    config::{
        APP_NAME, APP_TITLE, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, InstallMethod,
        PACKAGE_NAME, detect_install_method_from_paths, expand_tilde_path, get_agent_dir,
        get_auth_path, get_bin_dir, get_bundled_interactive_asset_path, get_changelog_path,
        get_custom_themes_dir, get_debug_log_path, get_docs_path, get_examples_path,
        get_export_template_dir, get_global_package_roots, get_interactive_assets_dir,
        get_models_path, get_package_dir, get_package_json_path, get_path_comparison_candidates,
        get_prompts_dir, get_readme_path, get_sessions_dir, get_settings_path,
        get_share_viewer_url, get_themes_dir, get_tools_dir, get_update_instruction_for_method,
        infer_pnpm_global_root, normalize_existing_path_for_comparison, read_command_output,
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
    assert_eq!(
        get_package_dir(),
        std::env::current_exe().unwrap().parent().unwrap()
    );
    assert_eq!(get_docs_path(), get_package_dir().join("docs"));
    assert_eq!(get_examples_path(), get_package_dir().join("examples"));
    assert_eq!(get_readme_path(), get_package_dir().join("README.md"));
    assert_eq!(get_changelog_path(), get_package_dir().join("CHANGELOG.md"));
    assert_eq!(
        get_package_json_path(),
        get_package_dir().join("package.json")
    );
    assert_eq!(get_themes_dir(), get_package_dir().join("theme"));
    assert_eq!(
        get_export_template_dir(),
        get_package_dir().join("export-html")
    );
    assert_eq!(
        get_interactive_assets_dir(),
        get_package_dir().join("assets")
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
fn normalizes_only_existing_path_comparison_candidates() {
    let existing = std::env::current_exe().unwrap();
    let expected = existing.canonicalize().unwrap();
    assert_eq!(
        normalize_existing_path_for_comparison(&existing, true),
        Some(expected.clone())
    );
    assert_eq!(get_path_comparison_candidates(&expected), vec![expected]);
    assert!(get_path_comparison_candidates(existing.with_extension("missing")).is_empty());
}

#[cfg(unix)]
#[test]
fn path_comparison_candidates_include_symlink_and_target() {
    let target = std::env::current_exe().unwrap().canonicalize().unwrap();
    let link = std::env::temp_dir().join(format!("zedflow-path-link-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&target, &link).unwrap();

    assert_eq!(
        get_path_comparison_candidates(&link),
        vec![link.clone(), target]
    );
    std::fs::remove_file(link).unwrap();
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
    assert_eq!(
        get_package_dir(),
        std::env::current_exe().unwrap().parent().unwrap()
    );
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

#[test]
#[cfg(unix)]
fn reads_command_output_and_discovers_configured_global_roots() {
    assert_eq!(
        read_command_output("sh", &["-c", "printf '  /global/root  \\n'"], true).unwrap(),
        Some("/global/root".to_owned())
    );
    assert_eq!(
        read_command_output("sh", &["-c", "exit 2"], false).unwrap(),
        None
    );
    assert_eq!(
        read_command_output("sh", &["-c", "printf failure >&2; exit 2"], true).unwrap_err(),
        "Failed to run sh -c printf failure >&2; exit 2: failure"
    );

    let npm_command = vec![
        "sh".to_owned(),
        "-c".to_owned(),
        "printf '/configured/root\\n'".to_owned(),
    ];
    assert_eq!(
        get_global_package_roots(InstallMethod::Npm, Some(&npm_command)).unwrap(),
        vec![PathBuf::from("/configured/root")]
    );
}

#[test]
fn discovers_pi_global_package_root_defaults() {
    assert!(
        get_global_package_roots(InstallMethod::Unknown, None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        get_global_package_roots(InstallMethod::BunBinary, None).unwrap(),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn infers_only_single_segment_pnpm_global_roots() {
    assert_eq!(
        infer_pnpm_global_root(&PathBuf::from(
            "/home/me/.local/share/pnpm/global/5/.pnpm/pkg/node_modules/pkg",
        )),
        Some(PathBuf::from("/home/me/.local/share/pnpm/global/5"))
    );
    assert_eq!(
        infer_pnpm_global_root(&PathBuf::from(
            "/home/me/.local/share/pnpm/global/5/nested/.pnpm/pkg/node_modules/pkg",
        )),
        None
    );
}

#[test]
fn detects_install_methods_and_formats_update_instructions_like_pi() {
    let package = PathBuf::from("C:\\Users\\Admin\\pnpm\\global\\5\\.pnpm\\pkg\\node_modules\\pkg");
    assert_eq!(
        detect_install_method_from_paths(&package, &PathBuf::from("node.exe"), false, false),
        InstallMethod::Pnpm
    );
    assert_eq!(
        detect_install_method_from_paths(
            &PathBuf::from("/home/me/.bun/install/global/node_modules/pkg"),
            &PathBuf::from("/usr/bin/node"),
            false,
            false,
        ),
        InstallMethod::Bun
    );
    assert_eq!(
        get_update_instruction_for_method(InstallMethod::Pnpm, PACKAGE_NAME),
        "Run: pnpm install -g --ignore-scripts --config.minimumReleaseAge=0 @earendil-works/pi-coding-agent"
    );
    assert_eq!(
        get_update_instruction_for_method(InstallMethod::Unknown, PACKAGE_NAME),
        "Update @earendil-works/pi-coding-agent using the package manager, wrapper, or source checkout that provides this installation."
    );
    assert_eq!(
        get_update_instruction_for_method(InstallMethod::BunBinary, PACKAGE_NAME),
        "Download from: https://github.com/earendil-works/pi-mono/releases/latest"
    );

    let prefix = std::env::temp_dir().join("pi npm prefix");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "inferred_npm_prefix_child"])
        .env(
            "PI_PACKAGE_DIR",
            prefix.join("lib/node_modules/@earendil-works/pi-coding-agent"),
        )
        .status()
        .unwrap();
    assert!(status.success());

    for (test, prefix) in [
        ("unquoted_npm_prefix_child", "pi'npm-prefix"),
        (
            "javascript_whitespace_npm_prefix_child",
            "pi\u{feff}npm-prefix",
        ),
        ("unicode_nel_npm_prefix_child", "pi\u{85}npm-prefix"),
    ] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", test])
            .env(
                "PI_PACKAGE_DIR",
                std::env::temp_dir()
                    .join(prefix)
                    .join("lib/node_modules/@earendil-works/pi-coding-agent"),
            )
            .status()
            .unwrap();
        assert!(status.success());
    }
}

#[test]
#[ignore]
fn inferred_npm_prefix_child() {
    let prefix = std::env::temp_dir().join("pi npm prefix");
    assert_eq!(
        get_update_instruction_for_method(InstallMethod::Npm, PACKAGE_NAME),
        format!(
            "Run: npm --prefix \"{}\" install -g --ignore-scripts --min-release-age=0 {PACKAGE_NAME}",
            prefix.display()
        )
    );
}

#[test]
#[ignore]
fn unquoted_npm_prefix_child() {
    assert_npm_prefix_display("pi'npm-prefix", false);
}

#[test]
#[ignore]
fn javascript_whitespace_npm_prefix_child() {
    assert_npm_prefix_display("pi\u{feff}npm-prefix", true);
}

#[test]
#[ignore]
fn unicode_nel_npm_prefix_child() {
    assert_npm_prefix_display("pi\u{85}npm-prefix", false);
}

fn assert_npm_prefix_display(name: &str, quoted: bool) {
    let prefix = std::env::temp_dir().join(name);
    let prefix = prefix.display();
    let prefix = if quoted {
        format!("\"{prefix}\"")
    } else {
        prefix.to_string()
    };
    assert_eq!(
        get_update_instruction_for_method(InstallMethod::Npm, PACKAGE_NAME),
        format!(
            "Run: npm --prefix {prefix} install -g --ignore-scripts --min-release-age=0 {PACKAGE_NAME}"
        )
    );
}
