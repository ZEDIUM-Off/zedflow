use zedflow_coding_agent::edit_diff::{detect_line_ending, normalize_to_lf, restore_line_endings};

#[test]
fn edit_normalization_round_trips_crlf() {
    let input = "one\r\ntwo\r\n";
    assert_eq!(detect_line_ending(input), "\r\n");
    assert_eq!(restore_line_endings(&normalize_to_lf(input), "\r\n"), input);
}
