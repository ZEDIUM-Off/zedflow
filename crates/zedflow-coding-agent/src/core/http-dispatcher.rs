use std::collections::HashMap;

pub const DEFAULT_HTTP_IDLE_TIMEOUT_MS: f64 = 300_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HttpIdleTimeoutChoice {
    pub label: &'static str,
    pub timeout_ms: f64,
}

pub const HTTP_IDLE_TIMEOUT_CHOICES: [HttpIdleTimeoutChoice; 5] = [
    HttpIdleTimeoutChoice {
        label: "30 sec",
        timeout_ms: 30_000.0,
    },
    HttpIdleTimeoutChoice {
        label: "1 min",
        timeout_ms: 60_000.0,
    },
    HttpIdleTimeoutChoice {
        label: "2 min",
        timeout_ms: 120_000.0,
    },
    HttpIdleTimeoutChoice {
        label: "5 min",
        timeout_ms: 300_000.0,
    },
    HttpIdleTimeoutChoice {
        label: "disabled",
        timeout_ms: 0.0,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HttpIdleTimeoutValue<'a> {
    String(&'a str),
    Number(f64),
    Other,
}

pub fn parse_http_idle_timeout_ms(value: HttpIdleTimeoutValue<'_>) -> Option<f64> {
    match value {
        HttpIdleTimeoutValue::String(value) => {
            let value = value.trim();
            if value.eq_ignore_ascii_case("disabled") {
                Some(0.0)
            } else if value.is_empty() {
                None
            } else {
                let number = if let Some(value) = value
                    .strip_prefix("0x")
                    .or_else(|| value.strip_prefix("0X"))
                {
                    parse_radix_f64(value, 16)?
                } else if let Some(value) = value
                    .strip_prefix("0b")
                    .or_else(|| value.strip_prefix("0B"))
                {
                    parse_radix_f64(value, 2)?
                } else if let Some(value) = value
                    .strip_prefix("0o")
                    .or_else(|| value.strip_prefix("0O"))
                {
                    parse_radix_f64(value, 8)?
                } else {
                    value.parse().ok()?
                };
                parse_http_idle_timeout_ms(HttpIdleTimeoutValue::Number(number))
            }
        }
        HttpIdleTimeoutValue::Number(value) if value.is_finite() && value >= 0.0 => {
            Some(value.floor())
        }
        _ => None,
    }
}

fn parse_radix_f64(value: &str, radix: u32) -> Option<f64> {
    if value.is_empty() {
        return None;
    }

    let mut limbs = vec![0_u32];
    for digit in value.chars().map(|character| character.to_digit(radix)) {
        let mut carry = digit? as u64;
        for limb in &mut limbs {
            carry += *limb as u64 * radix as u64;
            *limb = (carry % 1_000_000_000) as u32;
            carry /= 1_000_000_000;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
    }

    let mut decimal = limbs.pop()?.to_string();
    for limb in limbs.iter().rev() {
        decimal.push_str(&format!("{limb:09}"));
    }
    decimal.parse().ok()
}

pub fn format_http_idle_timeout_ms(timeout_ms: f64) -> String {
    HTTP_IDLE_TIMEOUT_CHOICES
        .iter()
        .find(|choice| choice.timeout_ms == timeout_ms)
        .map_or_else(
            || format!("{} sec", timeout_ms / 1000.0),
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
