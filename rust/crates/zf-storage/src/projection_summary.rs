//! Transport previews only. Canonical projections and detail endpoints keep exact errors.
use serde_json::{Value, json};

const ERROR_PREVIEW_CHARACTERS: usize = 512;

fn error(value: &mut Value) {
    let Some(original) = value.get("error") else {
        return;
    };
    if original.is_null() {
        value["errorTruncated"] = json!(false);
        value["errorCharacters"] = json!(0);
        return;
    }
    let Some(text) = original.as_str() else {
        return;
    };
    let characters = text.chars().count();
    // Transport adapters may receive a previously summarized delta. Preserve
    // its original size rather than treating the preview as the complete error.
    if characters == ERROR_PREVIEW_CHARACTERS
        && value["errorTruncated"] == true
        && value["errorCharacters"]
            .as_u64()
            .is_some_and(|count| count > ERROR_PREVIEW_CHARACTERS as u64)
    {
        return;
    }
    let truncated = characters > ERROR_PREVIEW_CHARACTERS;
    if truncated {
        value["error"] = json!(
            text.chars()
                .take(ERROR_PREVIEW_CHARACTERS)
                .collect::<String>()
        );
    }
    value["errorTruncated"] = json!(truncated);
    value["errorCharacters"] = json!(characters);
}

fn timeline_entry(value: &mut Value) {
    if value["kind"] == "tool" {
        error(&mut value["activity"]);
    }
}

/// Apply to an outgoing run clone, never to the canonical projection cache.
pub fn run(value: &mut Value) {
    error(value);
    for collection in ["activities", "toolActivities"] {
        for entry in value[collection].as_array_mut().into_iter().flatten() {
            error(entry);
        }
    }
    for entry in value["timeline"].as_array_mut().into_iter().flatten() {
        timeline_entry(entry);
    }
}

/// Also summarize historical delta rows at read time, without rewriting the journal.
pub fn operation(value: &mut Value) {
    match value["collection"].as_str() {
        Some("meta" | "activities" | "toolActivities") => error(&mut value["value"]),
        Some("timeline") => timeline_entry(&mut value["value"]),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_preview_is_bounded_without_changing_canonical_data_or_business_text() {
        let text = "é🦀漢".repeat(1000);
        let original = json!({"error":text,"activities":[{"error":text,"outputRef":"output"}],
            "toolActivities":[{"error":text,"callId":"call"}],
            "timeline":[{"kind":"tool","activity":{"error":text}},
                {"kind":"message","text":text,"error":text}],
            "composition":{"nodes":[{"config":{"error":text}}]}});
        let exact = serde_json::to_vec(&original).unwrap();
        let mut wire = original.clone();
        run(&mut wire);
        for summary in [
            &wire,
            &wire["activities"][0],
            &wire["toolActivities"][0],
            &wire["timeline"][0]["activity"],
        ] {
            assert_eq!(summary["error"].as_str().unwrap().chars().count(), 512);
            assert_eq!(summary["error"], text.chars().take(512).collect::<String>());
            assert_eq!(summary["errorTruncated"], true);
            assert_eq!(summary["errorCharacters"], 3000);
        }
        assert_eq!(wire["timeline"][1], original["timeline"][1]);
        assert_eq!(wire["composition"], original["composition"]);
        assert_eq!(wire["activities"][0]["outputRef"], "output");
        assert_eq!(serde_json::to_vec(&original).unwrap(), exact);
        assert!(serde_json::to_vec(&wire).unwrap().len() < exact.len());
        let once = wire.clone();
        run(&mut wire);
        assert_eq!(wire, once);
    }

    #[test]
    fn historic_operations_use_the_same_preview_and_keep_the_exact_journal_value() {
        let text = "failure".repeat(1000);
        for collection in ["meta", "activities", "toolActivities", "timeline"] {
            let entity = if collection == "timeline" {
                json!({"kind":"tool","activity":{"error":text,"callId":"call"}})
            } else {
                json!({"error":text,"callId":"call"})
            };
            let original = json!({"collection":collection,"id":"same-id","value":entity});
            let mut wire = original.clone();
            operation(&mut wire);
            let summary = if collection == "timeline" {
                &wire["value"]["activity"]
            } else {
                &wire["value"]
            };
            assert_eq!(summary["error"].as_str().unwrap().chars().count(), 512);
            assert_eq!(summary["errorCharacters"], 7000);
            assert_eq!(summary["errorTruncated"], true);
            assert_eq!(wire["id"], original["id"]);
            assert_eq!(original["value"], entity);
            let once = wire.clone();
            operation(&mut wire);
            assert_eq!(wire, once);
        }
    }

    #[test]
    fn small_and_cleared_errors_reset_preview_metadata_without_losing_text() {
        for text in [String::new(), "échec précis".into()] {
            let mut value = json!({"error":text,"errorTruncated":true,"errorCharacters":10000});
            error(&mut value);
            assert_eq!(value["error"], text);
            assert_eq!(value["errorCharacters"], text.chars().count());
            assert_eq!(value["errorTruncated"], false);
        }
        let mut boundary = json!({"error":"🦀".repeat(512)});
        error(&mut boundary);
        assert_eq!(boundary["errorCharacters"], 512);
        assert_eq!(boundary["errorTruncated"], false);
        let mut cleared = json!({"error":null,"errorTruncated":true,"errorCharacters":10000});
        error(&mut cleared);
        assert!(cleared["error"].is_null());
        assert_eq!(cleared["errorTruncated"], false);
        assert_eq!(cleared["errorCharacters"], 0);
        let mut unrelated = json!({"collection":"unknown","value":{"error":"x".repeat(5000)}});
        let original = unrelated.clone();
        operation(&mut unrelated);
        assert_eq!(unrelated, original);
    }
}
