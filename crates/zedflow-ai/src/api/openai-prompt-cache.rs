//! OpenAI prompt cache helpers ported from Pi.

/// Maximum number of Unicode scalar values in an OpenAI prompt cache key.
pub const OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH: usize = 64;

/// Clamps an optional OpenAI prompt cache key to Pi's provider limit.
///
/// `None` maps to TypeScript's `undefined`. Strings are truncated by Unicode
/// scalar value, matching JavaScript `Array.from(key).slice(0, 64).join("")`.
#[must_use]
pub fn clamp_openai_prompt_cache_key(key: Option<&str>) -> Option<String> {
    key.map(|key| {
        key.chars()
            .take(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_none_and_short_keys() {
        assert_eq!(clamp_openai_prompt_cache_key(None), None);
        assert_eq!(
            clamp_openai_prompt_cache_key(Some("cache-key")),
            Some("cache-key".to_string())
        );
    }

    #[test]
    fn clamps_by_unicode_scalar_values() {
        let key = format!("{}tail", "🦀".repeat(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH));

        assert_eq!(
            clamp_openai_prompt_cache_key(Some(&key)),
            Some("🦀".repeat(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH))
        );
    }
}
