use zedflow_coding_agent::utils::ansi::strip_ansi;

#[test]
fn strips_csi_and_osc_sequences() {
    assert_eq!(
        strip_ansi("\x1b[31mred\x1b[0m \x1b]8;;url\x07link\x1b]8;;\x07"),
        "red link"
    );
}
