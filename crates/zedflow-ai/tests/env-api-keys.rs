use std::process::Command;

use zedflow_ai::env_api_keys::{find_env_keys, get_env_api_key};

const CHILD_ENV: &str = "ZEDFLOW_ENV_API_KEYS_CHILD";
const API_KEY_ENV_NAMES: [&str; 4] = [
    "COPILOT_GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "ZAI_CODING_CN_API_KEY",
];

fn is_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

fn run_child(test_name: &str, env: &[(&str, &str)]) {
    let mut command = Command::new(std::env::current_exe().expect("test binary path is available"));
    command.arg("--exact").arg(test_name).arg("--nocapture");
    command.env(CHILD_ENV, "1");
    for name in API_KEY_ENV_NAMES {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }

    let status = command.status().expect("child test process runs");
    assert!(status.success(), "child test {test_name} failed: {status}");
}

#[test]
fn does_not_treat_generic_github_tokens_as_github_copilot_credentials() {
    run_child(
        "does_not_treat_generic_github_tokens_as_github_copilot_credentials_child",
        &[("GH_TOKEN", "gh-token"), ("GITHUB_TOKEN", "github-token")],
    );
}

#[test]
fn does_not_treat_generic_github_tokens_as_github_copilot_credentials_child() {
    if !is_child() {
        return;
    }

    assert_eq!(find_env_keys("github-copilot", None), None);
    assert_eq!(get_env_api_key("github-copilot", None), None);
}

#[test]
fn resolves_github_copilot_credentials_from_copilot_github_token() {
    run_child(
        "resolves_github_copilot_credentials_from_copilot_github_token_child",
        &[
            ("COPILOT_GITHUB_TOKEN", "copilot-token"),
            ("GH_TOKEN", "gh-token"),
            ("GITHUB_TOKEN", "github-token"),
        ],
    );
}

#[test]
fn resolves_github_copilot_credentials_from_copilot_github_token_child() {
    if !is_child() {
        return;
    }

    assert_eq!(
        find_env_keys("github-copilot", None),
        Some(vec!["COPILOT_GITHUB_TOKEN"])
    );
    assert_eq!(
        get_env_api_key("github-copilot", None),
        Some("copilot-token".to_owned())
    );
}

#[test]
fn resolves_zai_china_coding_plan_credentials_from_zai_coding_cn_api_key() {
    run_child(
        "resolves_zai_china_coding_plan_credentials_from_zai_coding_cn_api_key_child",
        &[("ZAI_CODING_CN_API_KEY", "zai-coding-cn-token")],
    );
}

#[test]
fn resolves_zai_china_coding_plan_credentials_from_zai_coding_cn_api_key_child() {
    if !is_child() {
        return;
    }

    assert_eq!(
        find_env_keys("zai-coding-cn", None),
        Some(vec!["ZAI_CODING_CN_API_KEY"])
    );
    assert_eq!(
        get_env_api_key("zai-coding-cn", None),
        Some("zai-coding-cn-token".to_owned())
    );
}
