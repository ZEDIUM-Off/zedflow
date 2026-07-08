//! HTTP header conversion helpers ported from Pi's `packages/ai/src/utils/headers.ts`.

use std::collections::HashMap;

use crate::types::ProviderHeaders;

/// Converts header key/value pairs into a string record.
///
/// This mirrors Pi's `headersToRecord`, which copies every entry from a Fetch
/// `Headers` object into a plain record.
#[must_use]
pub fn headers_to_record<I, K, V>(headers: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    headers
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect()
}

/// Converts optional provider headers into a string record, dropping suppressed headers.
///
/// Returns `None` when the input is absent or all entries are suppressed. A `None`
/// value in [`ProviderHeaders`] corresponds to Pi's `null`, which suppresses a
/// provider/API default header.
#[must_use]
pub fn provider_headers_to_record(
    headers: Option<&ProviderHeaders>,
) -> Option<HashMap<String, String>> {
    let result: HashMap<String, String> = headers?
        .iter()
        .filter_map(|(key, value)| value.as_ref().map(|value| (key.clone(), value.clone())))
        .collect();

    (!result.is_empty()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_all_header_entries() {
        let record = headers_to_record([
            ("content-type", "application/json"),
            ("x-request-id", "abc"),
        ]);

        assert_eq!(
            record.get("content-type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(record.get("x-request-id").map(String::as_str), Some("abc"));
    }

    #[test]
    fn drops_suppressed_provider_headers_and_empty_results() {
        let headers = HashMap::from([
            (
                "authorization".to_string(),
                Some("Bearer token".to_string()),
            ),
            ("x-default".to_string(), None),
        ]);

        let record = provider_headers_to_record(Some(&headers)).expect("one header remains");

        assert_eq!(record.len(), 1);
        assert_eq!(
            record.get("authorization").map(String::as_str),
            Some("Bearer token")
        );
        assert_eq!(provider_headers_to_record(None), None);
        assert_eq!(provider_headers_to_record(Some(&HashMap::new())), None);
    }
}
