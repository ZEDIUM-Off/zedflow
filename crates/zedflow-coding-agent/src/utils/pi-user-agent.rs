/// Formats the runtime identity sent to pi.dev.
#[must_use]
pub fn get_pi_user_agent(version: &str) -> String {
    format!(
        "pi/{version} ({}; rust/{}; {})",
        std::env::consts::OS,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH
    )
}
