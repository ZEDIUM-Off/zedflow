use zedflow_coding_agent::bash_executor::BashExecutorOptions;

#[test]
fn bash_execution_defaults_to_no_callback_or_cancellation() {
    let options = BashExecutorOptions::default();
    assert!(options.on_chunk.is_none());
    assert!(options.signal.is_none());
}
