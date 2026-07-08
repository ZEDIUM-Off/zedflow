//! Port of Pi `packages/ai/test/bedrock-utils.ts`.
//!
//! Test-only Amazon Bedrock credential helpers. These only inspect environment values;
//! they do not make live provider calls.

fn has_env_value(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

pub(crate) fn has_bedrock_credentials() -> bool {
    has_bedrock_credentials_from(
        has_env_value("AWS_PROFILE"),
        has_env_value("AWS_ACCESS_KEY_ID"),
        has_env_value("AWS_SECRET_ACCESS_KEY"),
        has_env_value("AWS_BEARER_TOKEN_BEDROCK"),
    )
}

fn has_bedrock_credentials_from(
    has_profile: bool,
    has_access_key_id: bool,
    has_secret_access_key: bool,
    has_bearer_token: bool,
) -> bool {
    has_profile || (has_access_key_id && has_secret_access_key) || has_bearer_token
}

#[test]
fn accepts_named_profile() {
    assert!(has_bedrock_credentials_from(true, false, false, false));
}

#[test]
fn accepts_iam_key_pair_only_when_both_parts_are_present() {
    assert!(has_bedrock_credentials_from(false, true, true, false));
    assert!(!has_bedrock_credentials_from(false, true, false, false));
    assert!(!has_bedrock_credentials_from(false, false, true, false));
}

#[test]
fn accepts_bedrock_bearer_token() {
    assert!(has_bedrock_credentials_from(false, false, false, true));
}

#[test]
fn rejects_missing_credentials() {
    assert!(!has_bedrock_credentials_from(false, false, false, false));
}

#[test]
fn reads_process_environment_without_live_calls() {
    let _ = has_bedrock_credentials();
}
