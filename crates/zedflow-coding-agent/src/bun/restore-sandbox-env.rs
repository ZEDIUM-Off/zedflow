//! Restores Bun's sandboxed environment from Linux's `/proc/self/environ`.

use std::{env, fs};

const PROC_SELF_ENVIRON: &str = "/proc/self/environ";

/// Restores the environment when `running_under_bun` and the current environment is empty.
///
/// Failures reading `/proc/self/environ` are ignored, as in Pi.
///
/// # Safety
/// The process environment must not be read or modified concurrently by other threads or
/// foreign code. Call this only during single-threaded runtime startup.
#[allow(unsafe_code)]
pub unsafe fn restore_sandbox_env(running_under_bun: bool) {
    if !running_under_bun || env::vars_os().next().is_some() {
        return;
    }

    if let Ok(data) = fs::read(PROC_SELF_ENVIRON) {
        for (key, value) in parse_sandbox_environ(&String::from_utf8_lossy(&data)) {
            // SAFETY: upheld by this function's safety contract.
            unsafe { env::set_var(key, value) };
        }
    }
}

/// Parses the NUL-delimited `KEY=VALUE` entries from `/proc/self/environ`.
/// Entries without a non-empty key are ignored.
pub fn parse_sandbox_environ(data: &str) -> Vec<(&str, &str)> {
    data.split('\0')
        .filter_map(|entry| {
            let index = entry.find('=')?;
            (index > 0).then(|| (&entry[..index], &entry[index + 1..]))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_sandbox_environ;

    #[test]
    fn parses_valid_entries_and_ignores_malformed_ones() {
        assert_eq!(
            parse_sandbox_environ("FOO=bar\0=missing-key\0no-equals\0BAZ=qu=x\0"),
            [("FOO", "bar"), ("BAZ", "qu=x")]
        );
    }
}
