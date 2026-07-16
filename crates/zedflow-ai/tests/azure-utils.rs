//! Port of Pi `packages/ai/test/azure-utils.ts`.

#![allow(
    dead_code,
    reason = "standalone port of helpers imported by Pi test suites"
)]
//!
//! Test-only Azure OpenAI helpers. These only inspect environment values and parse local strings;
//! they do not make live provider calls.

use std::collections::HashMap;

fn parse_deployment_name_map(value: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(value) = value else {
        return map;
    };

    for entry in value.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut parts = trimmed.split('=');
        let Some(model_id) = parts.next() else {
            continue;
        };
        let Some(deployment_name) = parts.next() else {
            continue;
        };
        if model_id.is_empty() || deployment_name.is_empty() {
            continue;
        }

        map.insert(
            model_id.trim().to_string(),
            deployment_name.trim().to_string(),
        );
    }

    map
}

fn has_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn has_env_value(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

pub(crate) fn has_azure_openai_credentials() -> bool {
    has_env_value("AZURE_OPENAI_API_KEY")
        && (has_env_value("AZURE_OPENAI_BASE_URL") || has_env_value("AZURE_OPENAI_RESOURCE_NAME"))
}

fn has_azure_openai_credentials_from(
    api_key: Option<&str>,
    base_url: Option<&str>,
    resource_name: Option<&str>,
) -> bool {
    has_value(api_key) && (has_value(base_url) || has_value(resource_name))
}

pub(crate) fn resolve_azure_deployment_name(model_id: &str) -> Option<String> {
    let map_value = std::env::var("AZURE_OPENAI_DEPLOYMENT_NAME_MAP")
        .ok()
        .filter(|value| !value.is_empty())?;
    resolve_azure_deployment_name_from(model_id, Some(&map_value))
}

fn resolve_azure_deployment_name_from(model_id: &str, map_value: Option<&str>) -> Option<String> {
    parse_deployment_name_map(map_value).get(model_id).cloned()
}

#[test]
fn parses_deployment_name_map_like_pi_helper() {
    let map = parse_deployment_name_map(Some(" gpt-5 = dep-a ,,bad,gpt-4= dep-b "));

    assert_eq!(map.get("gpt-5"), Some(&"dep-a".to_string()));
    assert_eq!(map.get("gpt-4"), Some(&"dep-b".to_string()));
    assert_eq!(map.len(), 2);
}

#[test]
fn keeps_typescript_split_limit_two_behavior() {
    let map = parse_deployment_name_map(Some("gpt-5=dep-a=ignored"));

    assert_eq!(map.get("gpt-5"), Some(&"dep-a".to_string()));
}

#[test]
fn checks_azure_credentials_without_live_calls() {
    assert!(has_azure_openai_credentials_from(
        Some("key"),
        Some("https://example.openai.azure.com"),
        None,
    ));
    assert!(has_azure_openai_credentials_from(
        Some("key"),
        None,
        Some("resource"),
    ));
    assert!(!has_azure_openai_credentials_from(
        Some(""),
        Some("https://example.openai.azure.com"),
        None,
    ));
    assert!(!has_azure_openai_credentials_from(Some("key"), None, None));
}

#[test]
fn resolves_deployment_name_from_map() {
    assert_eq!(
        resolve_azure_deployment_name_from("gpt-5", Some("gpt-5=dep-a,gpt-4=dep-b")),
        Some("dep-a".to_string())
    );
    assert_eq!(
        resolve_azure_deployment_name_from("missing", Some("gpt-5=dep-a")),
        None
    );
}
