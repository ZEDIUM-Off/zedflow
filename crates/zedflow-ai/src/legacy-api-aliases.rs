//! Deprecated legacy API aliases ported from Pi's `packages/ai/src/legacy-api-aliases.ts`.

/// Legacy stream alias metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyApiAlias {
    /// Deprecated export name.
    pub name: &'static str,
    /// Preferred API module.
    pub preferred_api: &'static str,
    /// Preferred function name.
    pub preferred_function: &'static str,
}

/// Deprecated stream aliases kept for callers migrating from the old flat API.
pub const LEGACY_API_ALIASES: &[LegacyApiAlias] = &[
    LegacyApiAlias {
        name: "streamAnthropic",
        preferred_api: "anthropic-messages",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleAnthropic",
        preferred_api: "anthropic-messages",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamAzureOpenAIResponses",
        preferred_api: "azure-openai-responses",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleAzureOpenAIResponses",
        preferred_api: "azure-openai-responses",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamGoogle",
        preferred_api: "google-generative-ai",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleGoogle",
        preferred_api: "google-generative-ai",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamGoogleVertex",
        preferred_api: "google-vertex",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleGoogleVertex",
        preferred_api: "google-vertex",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamMistral",
        preferred_api: "mistral-conversations",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleMistral",
        preferred_api: "mistral-conversations",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamOpenAICodexResponses",
        preferred_api: "openai-codex-responses",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleOpenAICodexResponses",
        preferred_api: "openai-codex-responses",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamOpenAICompletions",
        preferred_api: "openai-completions",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleOpenAICompletions",
        preferred_api: "openai-completions",
        preferred_function: "streamSimple",
    },
    LegacyApiAlias {
        name: "streamOpenAIResponses",
        preferred_api: "openai-responses",
        preferred_function: "stream",
    },
    LegacyApiAlias {
        name: "streamSimpleOpenAIResponses",
        preferred_api: "openai-responses",
        preferred_function: "streamSimple",
    },
];

/// Finds a deprecated alias by export name.
#[must_use]
pub fn legacy_api_alias(name: &str) -> Option<&'static LegacyApiAlias> {
    LEGACY_API_ALIASES.iter().find(|alias| alias.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_legacy_alias() {
        let alias = legacy_api_alias("streamOpenAIResponses").expect("alias exists");
        assert_eq!(alias.preferred_api, "openai-responses");
    }
}
