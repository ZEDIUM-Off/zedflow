use std::io::Cursor;
use zedflow_coding_agent::run_rpc_loop;

#[test]
fn rpc_loop_stops_cleanly_when_its_input_process_exits() {
    let mut output = Vec::new();
    run_rpc_loop(Cursor::new(b""), &mut output).unwrap();

    assert!(output.is_empty());
}
