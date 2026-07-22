use std::collections::HashMap;

pub const DEFAULT_HTTP_IDLE_TIMEOUT_MS: u64 = 300_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpIdleTimeoutChoice {
    pub label: &'static str,
    pub timeout_ms: u64,
}

pub const HTTP_IDLE_TIMEOUT_CHOICES: [HttpIdleTimeoutChoice; 5] = [
    HttpIdleTimeoutChoice {
        label: "30 sec",
        timeout_ms: 30_000,
    },
    HttpIdleTimeoutChoice {
        label: "1 min",
        timeout_ms: 60_000,
    },
    HttpIdleTimeoutChoice {
        label: "2 min",
        timeout_ms: 120_000,
    },
    HttpIdleTimeoutChoice {
        label: "5 min",
        timeout_ms: 300_000,
    },
    HttpIdleTimeoutChoice {
        label: "disabled",
        timeout_ms: 0,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HttpIdleTimeoutValue<'a> {
    String(&'a str),
    Number(f64),
    Other,
}

pub fn parse_http_idle_timeout_ms(value: HttpIdleTimeoutValue<'_>) -> Option<u64> {
    match value {
        HttpIdleTimeoutValue::String(value) => {
            let value = value.trim();
            if value.eq_ignore_ascii_case("disabled") {
                Some(0)
            } else if value.is_empty() {
                None
            } else {
                let number = if let Some(value) = value
                    .strip_prefix("0x")
                    .or_else(|| value.strip_prefix("0X"))
                {
                    u64::from_str_radix(value, 16).ok()? as f64
                } else if let Some(value) = value
                    .strip_prefix("0b")
                    .or_else(|| value.strip_prefix("0B"))
                {
                    u64::from_str_radix(value, 2).ok()? as f64
                } else if let Some(value) = value
                    .strip_prefix("0o")
                    .or_else(|| value.strip_prefix("0O"))
                {
                    u64::from_str_radix(value, 8).ok()? as f64
                } else {
                    value.parse().ok()?
                };
                parse_http_idle_timeout_ms(HttpIdleTimeoutValue::Number(number))
            }
        }
        HttpIdleTimeoutValue::Number(value) if value.is_finite() && value >= 0.0 => {
            Some(value.floor() as u64)
        }
        _ => None,
    }
}

pub fn format_http_idle_timeout_ms(timeout_ms: u64) -> String {
    HTTP_IDLE_TIMEOUT_CHOICES
        .iter()
        .find(|choice| choice.timeout_ms == timeout_ms)
        .map_or_else(
            || format!("{} sec", timeout_ms as f64 / 1000.0),
            |choice| choice.label.to_owned(),
        )
}

pub fn apply_http_proxy_settings(http_proxy: Option<&str>, env: &mut HashMap<String, String>) {
    let Some(proxy) = http_proxy.map(str::trim).filter(|proxy| !proxy.is_empty()) else {
        return;
    };
    env.entry("HTTP_PROXY".into())
        .or_insert_with(|| proxy.into());
    env.entry("HTTPS_PROXY".into())
        .or_insert_with(|| proxy.into());
}
