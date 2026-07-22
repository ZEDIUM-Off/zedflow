use std::collections::HashMap;

use zedflow_coding_agent::http_dispatcher::{
    HttpIdleTimeoutValue, apply_http_proxy_settings, format_http_idle_timeout_ms,
    parse_http_idle_timeout_ms,
};

#[test]
fn parses_and_formats_idle_timeouts() {
    assert_eq!(
        parse_http_idle_timeout_ms(HttpIdleTimeoutValue::String(" DISABLED ")),
        Some(0.0)
    );
    assert_eq!(
        parse_http_idle_timeout_ms(HttpIdleTimeoutValue::String("1200.9")),
        Some(1200.0)
    );
    assert_eq!(
        parse_http_idle_timeout_ms(HttpIdleTimeoutValue::String("0x10")),
        Some(16.0)
    );
    assert_eq!(
        parse_http_idle_timeout_ms(HttpIdleTimeoutValue::Number(f64::INFINITY)),
        None
    );
    assert_eq!(
        parse_http_idle_timeout_ms(HttpIdleTimeoutValue::Number(1e30)),
        Some(1e30)
    );
    assert_eq!(format_http_idle_timeout_ms(60_000.0), "1 min");
    assert_eq!(format_http_idle_timeout_ms(1_500.0), "1.5 sec");
}

#[test]
fn proxy_settings_fill_only_missing_environment_defaults() {
    let mut env = HashMap::from([("HTTP_PROXY".into(), "http://existing:8080".into())]);

    apply_http_proxy_settings(Some(" http://settings:7890 "), &mut env);

    assert_eq!(env["HTTP_PROXY"], "http://existing:8080");
    assert_eq!(env["HTTPS_PROXY"], "http://settings:7890");

    let before = env.clone();
    apply_http_proxy_settings(Some("   "), &mut env);
    assert_eq!(env, before);
}
