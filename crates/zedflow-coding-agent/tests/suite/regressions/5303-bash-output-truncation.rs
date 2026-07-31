use zedflow_coding_agent::truncate::{TruncatedBy, TruncationOptions, truncate_head};

#[test]
fn truncation_reports_the_limit_that_discarded_output() {
    let result = truncate_head(
        "one\ntwo",
        TruncationOptions {
            max_lines: 1,
            max_bytes: 100,
        },
    );
    assert_eq!(result.content, "one");
    assert_eq!(result.truncated_by, Some(TruncatedBy::Lines));
}
