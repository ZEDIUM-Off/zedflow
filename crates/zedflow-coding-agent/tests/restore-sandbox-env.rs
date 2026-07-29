use zedflow_coding_agent::bun_restore_sandbox_env::parse_sandbox_environ;
#[test]
fn parses_nul_delimited_environment_without_accepting_invalid_keys() {
    assert_eq!(
        parse_sandbox_environ("A=1\0=bad\0missing\0B=x=y\0"),
        [("A", "1"), ("B", "x=y")]
    );
}
