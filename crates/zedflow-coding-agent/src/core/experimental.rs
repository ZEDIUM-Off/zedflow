/// Returns whether Pi's experimental feature gate is enabled.
pub fn are_experimental_features_enabled() -> bool {
    std::env::var_os("PI_EXPERIMENTAL").is_some_and(|value| value == "1")
}
