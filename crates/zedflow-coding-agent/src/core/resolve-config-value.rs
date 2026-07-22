use std::{
    collections::HashMap,
    fmt,
    io::Read,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

static COMMAND_RESULT_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

type Env<'a> = Option<&'a HashMap<String, String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplatePart {
    Literal(String),
    Env(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValueError(String);

impl fmt::Display for ConfigValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigValueError {}

fn is_env_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_env_continue(byte: u8) -> bool {
    is_env_start(byte) || byte.is_ascii_digit()
}

fn append_literal(parts: &mut Vec<TemplatePart>, value: &str) {
    if value.is_empty() {
        return;
    }
    if let Some(TemplatePart::Literal(previous)) = parts.last_mut() {
        previous.push_str(value);
    } else {
        parts.push(TemplatePart::Literal(value.to_owned()));
    }
}

fn parse_template(config: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut index = 0;
    while let Some(relative) = config[index..].find('$') {
        let dollar = index + relative;
        append_literal(&mut parts, &config[index..dollar]);
        let after = dollar + 1;
        match config.as_bytes().get(after).copied() {
            Some(b'$' | b'!') => {
                append_literal(&mut parts, &config[after..after + 1]);
                index = after + 1;
            }
            Some(b'{') => {
                if let Some(relative_end) = config[after + 1..].find('}') {
                    let end = after + 1 + relative_end;
                    let name = &config[after + 1..end];
                    if name
                        .as_bytes()
                        .first()
                        .is_some_and(|byte| is_env_start(*byte))
                        && name.as_bytes()[1..]
                            .iter()
                            .all(|byte| is_env_continue(*byte))
                    {
                        parts.push(TemplatePart::Env(name.to_owned()));
                    } else {
                        append_literal(&mut parts, &config[dollar..=end]);
                    }
                    index = end + 1;
                } else {
                    append_literal(&mut parts, "$");
                    index = after;
                }
            }
            Some(byte) if is_env_start(byte) => {
                let mut end = after + 1;
                while config
                    .as_bytes()
                    .get(end)
                    .is_some_and(|byte| is_env_continue(*byte))
                {
                    end += 1;
                }
                parts.push(TemplatePart::Env(config[after..end].to_owned()));
                index = end;
            }
            _ => {
                append_literal(&mut parts, "$");
                index = after;
            }
        }
    }
    append_literal(&mut parts, &config[index..]);
    parts
}

fn env_value(name: &str, env: Env<'_>) -> Option<String> {
    env.and_then(|values| values.get(name))
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

fn resolve_template(parts: &[TemplatePart], env: Env<'_>) -> Option<String> {
    let mut resolved = String::new();
    for part in parts {
        match part {
            TemplatePart::Literal(value) => resolved.push_str(value),
            TemplatePart::Env(name) => resolved.push_str(&env_value(name, env)?),
        }
    }
    Some(resolved)
}

pub fn get_config_value_env_var_name(config: &str) -> Option<String> {
    if is_command_config_value(config) {
        return None;
    }
    match parse_template(config).as_slice() {
        [TemplatePart::Env(name)] => Some(name.clone()),
        _ => None,
    }
}

pub fn get_config_value_env_var_names(config: &str) -> Vec<String> {
    if is_command_config_value(config) {
        return Vec::new();
    }
    let mut names = Vec::new();
    for part in parse_template(config) {
        if let TemplatePart::Env(name) = part
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

pub fn get_missing_config_value_env_var_names(config: &str, env: Env<'_>) -> Vec<String> {
    get_config_value_env_var_names(config)
        .into_iter()
        .filter(|name| env_value(name, env).is_none())
        .collect()
}

pub fn is_command_config_value(config: &str) -> bool {
    config.starts_with('!')
}

pub fn is_config_value_configured(config: &str, env: Env<'_>) -> bool {
    get_missing_config_value_env_var_names(config, env).is_empty()
}

fn execute_command_uncached(config: &str) -> Option<String> {
    let command = &config[1..];
    #[cfg(windows)]
    let mut child = Command::new("cmd");
    #[cfg(windows)]
    child.args(["/C", command]);
    #[cfg(not(windows))]
    let mut child = Command::new("/bin/sh");
    #[cfg(not(windows))]
    child.args(["-c", command]);

    let mut child = child
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.take(1_048_577).read_to_string(&mut output);
        output
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let output = reader.join().ok()?;
    if output.len() > 1_048_576 {
        return None;
    }
    let value = output.trim().to_owned();
    (success && !value.is_empty()).then_some(value)
}

fn execute_command(config: &str) -> Option<String> {
    let cache = COMMAND_RESULT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(value) = cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(config)
    {
        return value.clone();
    }
    let value = execute_command_uncached(config);
    cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(config.to_owned(), value.clone());
    value
}

pub fn resolve_config_value(config: &str, env: Env<'_>) -> Option<String> {
    if is_command_config_value(config) {
        execute_command(config)
    } else {
        resolve_template(&parse_template(config), env)
    }
}

pub fn resolve_config_value_uncached(config: &str, env: Env<'_>) -> Option<String> {
    if is_command_config_value(config) {
        execute_command_uncached(config)
    } else {
        resolve_template(&parse_template(config), env)
    }
}

pub fn resolve_config_value_or_throw(
    config: &str,
    description: &str,
    env: Env<'_>,
) -> Result<String, ConfigValueError> {
    if let Some(value) = resolve_config_value_uncached(config, env) {
        return Ok(value);
    }
    if is_command_config_value(config) {
        return Err(ConfigValueError(format!(
            "Failed to resolve {description} from shell command: {}",
            &config[1..]
        )));
    }
    let missing = get_missing_config_value_env_var_names(config, env);
    let suffix = match missing.as_slice() {
        [name] => format!(" from environment variable: {name}"),
        [_, ..] => format!(" from environment variables: {}", missing.join(", ")),
        [] => String::new(),
    };
    Err(ConfigValueError(format!(
        "Failed to resolve {description}{suffix}"
    )))
}

pub fn resolve_headers(
    headers: Option<&HashMap<String, String>>,
    env: Env<'_>,
) -> Option<HashMap<String, String>> {
    let resolved: HashMap<_, _> = headers?
        .iter()
        .filter_map(|(key, value)| {
            resolve_config_value(value, env)
                .filter(|value| !value.is_empty())
                .map(|value| (key.clone(), value))
        })
        .collect();
    (!resolved.is_empty()).then_some(resolved)
}

pub fn resolve_headers_or_throw(
    headers: Option<&HashMap<String, String>>,
    description: &str,
    env: Env<'_>,
) -> Result<Option<HashMap<String, String>>, ConfigValueError> {
    let Some(headers) = headers else {
        return Ok(None);
    };
    let mut resolved = HashMap::new();
    for (key, value) in headers {
        resolved.insert(
            key.clone(),
            resolve_config_value_or_throw(value, &format!("{description} header \"{key}\""), env)?,
        );
    }
    Ok((!resolved.is_empty()).then_some(resolved))
}

pub fn clear_config_value_cache() {
    COMMAND_RESULT_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
}
