use zedflow_ai::api::google_shared::{Part, is_thinking_part, retain_thought_signature};

#[test]
fn only_thought_true_marks_thinking() {
    assert!(is_thinking_part(&Part {
        thought: Some(true),
        ..Part::default()
    }));
    assert!(is_thinking_part(&Part {
        thought: Some(true),
        thought_signature: Some("opaque".into()),
        ..Part::default()
    }));
    assert!(!is_thinking_part(&Part {
        thought_signature: Some("opaque".into()),
        ..Part::default()
    }));
    assert!(!is_thinking_part(&Part {
        thought: Some(false),
        thought_signature: Some("opaque".into()),
        ..Part::default()
    }));
}

#[test]
fn signatures_persist_across_omitted_deltas_and_non_empty_values_replace() {
    let first = retain_thought_signature(None, Some("sig-1"));
    assert_eq!(retain_thought_signature(first, None), Some("sig-1"));
    assert_eq!(retain_thought_signature(first, Some("")), Some("sig-1"));
    assert_eq!(
        retain_thought_signature(first, Some("sig-2")),
        Some("sig-2")
    );
}
