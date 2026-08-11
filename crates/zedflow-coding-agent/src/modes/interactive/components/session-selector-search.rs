//! Pi-compatible session-selector search and sorting.

use std::time::SystemTime;

use regex::{Regex, RegexBuilder};
use zedflow_tui::fuzzy_match;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Threaded,
    Recent,
    Relevance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NameFilter {
    #[default]
    All,
    Named,
}

/// Searchable session data used by the interactive session selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub all_messages_text: String,
    pub cwd: String,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTokenKind {
    Fuzzy,
    Phrase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchToken {
    pub kind: SearchTokenKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Tokens,
    Regex,
}

#[derive(Debug, Clone)]
pub struct ParsedSearchQuery {
    pub mode: SearchMode,
    pub tokens: Vec<SearchToken>,
    pub regex: Option<Regex>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchResult {
    pub matches: bool,
    pub score: f64,
}

fn normalize_whitespace_lower(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn session_search_text(session: &SessionInfo) -> String {
    format!(
        "{} {} {} {}",
        session.id,
        session.name.as_deref().unwrap_or_default(),
        session.all_messages_text,
        session.cwd
    )
}

#[must_use]
pub fn has_session_name(session: &SessionInfo) -> bool {
    session
        .name
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty())
}

#[must_use]
pub fn parse_search_query(query: &str) -> ParsedSearchQuery {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return ParsedSearchQuery {
            mode: SearchMode::Tokens,
            tokens: Vec::new(),
            regex: None,
            error: None,
        };
    }

    if let Some(pattern) = trimmed.strip_prefix("re:") {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return ParsedSearchQuery {
                mode: SearchMode::Regex,
                tokens: Vec::new(),
                regex: None,
                error: Some("Empty regex".into()),
            };
        }
        return match RegexBuilder::new(pattern).case_insensitive(true).build() {
            Ok(regex) => ParsedSearchQuery {
                mode: SearchMode::Regex,
                tokens: Vec::new(),
                regex: Some(regex),
                error: None,
            },
            Err(error) => ParsedSearchQuery {
                mode: SearchMode::Regex,
                tokens: Vec::new(),
                regex: None,
                error: Some(error.to_string()),
            },
        };
    }

    let mut tokens = Vec::new();
    let mut buffer = String::new();
    let mut in_quote = false;
    for character in trimmed.chars() {
        if character == '"' {
            flush_token(
                &mut tokens,
                if in_quote {
                    SearchTokenKind::Phrase
                } else {
                    SearchTokenKind::Fuzzy
                },
                &mut buffer,
            );
            in_quote = !in_quote;
        } else if !in_quote && character.is_whitespace() {
            flush_token(&mut tokens, SearchTokenKind::Fuzzy, &mut buffer);
        } else {
            buffer.push(character);
        }
    }

    if in_quote {
        tokens = trimmed
            .split_whitespace()
            .map(|value| SearchToken {
                kind: SearchTokenKind::Fuzzy,
                value: value.to_owned(),
            })
            .collect();
    } else {
        flush_token(&mut tokens, SearchTokenKind::Fuzzy, &mut buffer);
    }

    ParsedSearchQuery {
        mode: SearchMode::Tokens,
        tokens,
        regex: None,
        error: None,
    }
}

fn flush_token(tokens: &mut Vec<SearchToken>, kind: SearchTokenKind, buffer: &mut String) {
    let value = buffer.trim();
    if !value.is_empty() {
        tokens.push(SearchToken {
            kind,
            value: value.to_owned(),
        });
    }
    buffer.clear();
}

#[must_use]
pub fn match_session(session: &SessionInfo, parsed: &ParsedSearchQuery) -> MatchResult {
    let text = session_search_text(session);
    if parsed.mode == SearchMode::Regex {
        let Some(regex) = &parsed.regex else {
            return MatchResult {
                matches: false,
                score: 0.0,
            };
        };
        return regex.find(&text).map_or(
            MatchResult {
                matches: false,
                score: 0.0,
            },
            |found| MatchResult {
                matches: true,
                // JavaScript's String#search index is counted in UTF-16 code units.
                score: text[..found.start()].encode_utf16().count() as f64 * 0.1,
            },
        );
    }

    let mut score = 0.0;
    let mut normalized_text = None;
    for token in &parsed.tokens {
        if token.kind == SearchTokenKind::Phrase {
            let text = normalized_text.get_or_insert_with(|| normalize_whitespace_lower(&text));
            let phrase = normalize_whitespace_lower(&token.value);
            if phrase.is_empty() {
                continue;
            }
            let Some(index) = text.find(&phrase) else {
                return MatchResult {
                    matches: false,
                    score: 0.0,
                };
            };
            score += text[..index].encode_utf16().count() as f64 * 0.1;
        } else {
            let result = fuzzy_match(&token.value, &text);
            if !result.matches {
                return MatchResult {
                    matches: false,
                    score: 0.0,
                };
            }
            score += result.score;
        }
    }
    MatchResult {
        matches: true,
        score,
    }
}

#[must_use]
pub fn filter_and_sort_sessions(
    sessions: &[SessionInfo],
    query: &str,
    sort_mode: SortMode,
    name_filter: NameFilter,
) -> Vec<SessionInfo> {
    let sessions: Vec<_> = sessions
        .iter()
        .filter(|session| name_filter == NameFilter::All || has_session_name(session))
        .collect();
    if query.trim().is_empty() {
        return sessions.into_iter().cloned().collect();
    }

    let parsed = parse_search_query(query);
    if parsed.error.is_some() {
        return Vec::new();
    }

    if sort_mode == SortMode::Recent {
        return sessions
            .into_iter()
            .filter(|session| match_session(session, &parsed).matches)
            .cloned()
            .collect();
    }

    let mut scored: Vec<_> = sessions
        .into_iter()
        .filter_map(|session| {
            let result = match_session(session, &parsed);
            result.matches.then_some((session, result.score))
        })
        .collect();
    scored.sort_by(|(left, left_score), (right, right_score)| {
        left_score
            .total_cmp(right_score)
            .then_with(|| right.modified.cmp(&left.modified))
    });
    scored
        .into_iter()
        .map(|(session, _)| session.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    fn session(id: &str, name: Option<&str>, text: &str, modified: u64) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            name: name.map(str::to_owned),
            all_messages_text: text.into(),
            cwd: String::new(),
            modified: UNIX_EPOCH + Duration::from_secs(modified),
        }
    }

    #[test]
    fn search_matches_phrases_regex_names_and_sorting() {
        let sessions = vec![
            session("late", Some("Project"), "xxxx brave node\n cve", 3),
            session("early", None, "brave node cve", 1),
            session("other", Some("   "), "other", 2),
        ];

        assert_eq!(
            filter_and_sort_sessions(&sessions, "\"node cve\"", SortMode::Recent, NameFilter::All)
                .into_iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            ["late", "early"]
        );
        assert_eq!(
            filter_and_sort_sessions(&sessions, "re:^late", SortMode::Recent, NameFilter::All)
                .into_iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            ["late"]
        );
        assert_eq!(
            filter_and_sort_sessions(&sessions, "brave", SortMode::Relevance, NameFilter::Named)
                .into_iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            ["late"]
        );
    }
}
