use std::collections::HashMap;
use zedflow_coding_agent::resolve_config_value::*;

fn env(values: &[(&str, &str)]) -> HashMap<String, String> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn parses_and_resolves_templates_like_pi() {
    let values = env(&[("TOKEN", "secret"), ("EMPTY", "")]);
    assert_eq!(
        resolve_config_value("Bearer $TOKEN/${TOKEN} $$ $!", Some(&values)),
        Some("Bearer secret/secret $ !".into())
    );
    assert_eq!(
        get_config_value_env_var_name("$TOKEN"),
        Some("TOKEN".into())
    );
    assert_eq!(
        get_config_value_env_var_names("$TOKEN/${TOKEN}/$OTHER/$TOKEN"),
        ["TOKEN", "OTHER"]
    );
    assert_eq!(
        get_missing_config_value_env_var_names("$TOKEN/$EMPTY/$OTHER", Some(&values)),
        ["EMPTY", "OTHER"]
    );
    assert_eq!(
        resolve_config_value("${not-valid}", Some(&values)),
        Some("${not-valid}".into())
    );
}

#[test]
fn resolves_headers_and_reports_the_missing_name() {
    let values = env(&[("TOKEN", "secret")]);
    let headers = HashMap::from([
        ("Authorization".into(), "Bearer $TOKEN".into()),
        ("Missing".into(), "$NOPE".into()),
    ]);
    assert_eq!(
        resolve_headers(Some(&headers), Some(&values)).unwrap(),
        HashMap::from([("Authorization".into(), "Bearer secret".into())])
    );
    assert_eq!(
        resolve_headers_or_throw(Some(&headers), "provider", Some(&values))
            .unwrap_err()
            .to_string(),
        "Failed to resolve provider header \"Missing\" from environment variable: NOPE"
    );
}

#[cfg(not(windows))]
#[test]
fn commands_are_trimmed_and_cached() {
    clear_config_value_cache();
    let path = std::env::temp_dir().join(format!("zedflow-config-cache-{}", std::process::id()));
    let command = format!(
        "!n=$(cat {0} 2>/dev/null || echo 0); n=$((n+1)); echo $n > {0}; echo $n",
        path.display()
    );
    assert_eq!(resolve_config_value(&command, None), Some("1".into()));
    assert_eq!(resolve_config_value(&command, None), Some("1".into()));
    assert_eq!(
        resolve_config_value_uncached(&command, None),
        Some("2".into())
    );
    let _ = std::fs::remove_file(path);
}

#[cfg(not(windows))]
#[test]
fn command_output_over_default_buffer_is_rejected() {
    assert_eq!(
        resolve_config_value_uncached("!head -c 1048577 /dev/zero | tr '\\0' x", None),
        None
    );
}
