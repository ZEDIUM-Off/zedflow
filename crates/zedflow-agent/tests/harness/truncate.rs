use zedflow_agent::harness::utils::truncate::{
    TruncatedBy, TruncationOptions, truncate_head, truncate_tail,
};

fn byte_len(content: &str) -> usize {
    content.len()
}

fn utf8_tail(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    let mut start = content.len() - max_bytes;
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    content[start..].to_string()
}

fn options(max_bytes: usize, max_lines: usize) -> TruncationOptions {
    TruncationOptions {
        max_bytes,
        max_lines,
    }
}

fn assert_matches_utf8_tail(input: &str, max_byte_values: Option<&[usize]>) {
    let sampled;
    let values = match max_byte_values {
        Some(values) => values,
        None => {
            sampled = (0..input.len() + 5).collect::<Vec<_>>();
            &sampled
        }
    };

    for &max_bytes in values {
        let result = truncate_tail(input, Some(options(max_bytes, 10)));
        let expected = utf8_tail(input, max_bytes);
        assert_eq!(
            result.content, expected,
            "tail mismatch input={input:?} max_bytes={max_bytes}"
        );
        assert!(
            result.output_bytes <= max_bytes,
            "tail output exceeded byte limit input={input:?} max_bytes={max_bytes} output_bytes={}",
            result.output_bytes
        );
    }
}

fn sampled_byte_limits(input: &str) -> Vec<usize> {
    let total_bytes = input.len();
    let candidates = [
        0,
        1,
        2,
        3,
        4,
        5,
        8,
        (total_bytes / 2).saturating_sub(1),
        total_bytes / 2,
        total_bytes / 2 + 1,
        total_bytes.saturating_sub(8),
        total_bytes.saturating_sub(5),
        total_bytes.saturating_sub(4),
        total_bytes.saturating_sub(3),
        total_bytes.saturating_sub(2),
        total_bytes.saturating_sub(1),
        total_bytes,
        total_bytes + 1,
        total_bytes + 4,
    ];
    let mut values = candidates.to_vec();
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn counts_utf8_bytes_without_node_buffer() {
    let content = "aé🙂\nb";
    let result = truncate_head(content, Some(options(100, 10)));

    assert!(!result.truncated);
    assert_eq!(result.total_bytes, byte_len(content));
    assert_eq!(result.output_bytes, byte_len(content));
    assert_eq!(result.total_bytes, 9);
}

#[test]
fn truncates_head_on_utf8_byte_limits_without_partial_lines() {
    let result = truncate_head("éé\nabc", Some(options(4, 10)));

    assert_eq!(result.content, "éé");
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::Bytes));
    assert_eq!(result.output_bytes, 4);
    assert!(!result.first_line_exceeds_limit);
}

#[test]
fn reports_head_truncation_when_the_first_line_exceeds_the_byte_limit() {
    let result = truncate_head("éé\nabc", Some(options(3, 10)));

    assert_eq!(result.content, "");
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::Bytes));
    assert!(result.first_line_exceeds_limit);
}

#[test]
fn truncates_tail_on_utf8_boundaries_when_only_a_partial_last_line_fits() {
    let result = truncate_tail("aé🙂b", Some(options(5, 10)));

    assert_eq!(result.content, "🙂b");
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::Bytes));
    assert!(result.last_line_partial);
    assert_eq!(result.output_bytes, 5);
}

#[test]
fn truncates_an_oversized_single_line_with_a_trailing_newline() {
    let input = format!("{}\n", "X".repeat(300_000));
    let result = truncate_tail(&input, Some(options(1024, 100)));

    assert_eq!(result.content, "X".repeat(1024));
    assert_eq!(result.output_bytes, 1024);
    assert_eq!(result.output_lines, 1);
    assert!(result.last_line_partial);
    assert_eq!(result.truncated_by, Some(TruncatedBy::Bytes));
}

#[test]
fn drops_an_oversized_trailing_character_when_it_cannot_fit_in_tail_byte_limit() {
    let result = truncate_tail("abc🙂", Some(options(3, 10)));

    assert_eq!(result.content, "");
    assert!(result.truncated);
    assert_eq!(result.truncated_by, Some(TruncatedBy::Bytes));
    assert!(result.last_line_partial);
    assert_eq!(result.output_bytes, 0);
}

#[ignore = "JS-only: Rust str cannot contain lone UTF-16 surrogate code units used by Pi Buffer edge cases"]
#[test]
fn matches_node_buffer_tail_truncation_for_lone_surrogate_edge_cases() {}

#[test]
fn matches_utf8_tail_truncation_semantics_across_deterministic_fuzz_cases() {
    let alphabet = [
        "a",
        "\u{007f}",
        "\u{0080}",
        "é",
        "\u{07ff}",
        "\u{0800}",
        "中",
        "\u{d7ff}",
        "🙂",
        "\u{e000}",
        "\u{ffff}",
        "👩\u{200d}💻",
    ];

    fn check_exhaustive(prefix: &str, depth: usize, alphabet: &[&str]) {
        let limits = sampled_byte_limits(prefix);
        assert_matches_utf8_tail(prefix, Some(&limits));
        if depth == 0 {
            return;
        }
        for character in alphabet {
            check_exhaustive(&format!("{prefix}{character}"), depth - 1, alphabet);
        }
    }
    check_exhaustive("", 3, &alphabet);

    let mut seed = 0x12345678u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed as f64 / 0x1_0000_0000u64 as f64
    };
    for _ in 0..1_000 {
        let mut input = String::new();
        let length = (random() * 80.0).floor() as usize;
        for _ in 0..length {
            let index = (random() * alphabet.len() as f64).floor() as usize;
            input.push_str(alphabet[index]);
        }
        let limits = sampled_byte_limits(&input);
        assert_matches_utf8_tail(&input, Some(&limits));
    }
}
