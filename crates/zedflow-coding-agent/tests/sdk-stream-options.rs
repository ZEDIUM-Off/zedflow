use zedflow_ai::types::SimpleStreamOptions;
use zedflow_coding_agent::sdk::apply_stream_timeout_defaults;

#[test]
fn sdk_stream_timeouts_forward_settings_and_preserve_request_overrides() {
    let settings = apply_stream_timeout_defaults(SimpleStreamOptions::default(), 1234, 5678);
    assert_eq!(settings.stream.timeout_ms, Some(1234));
    assert_eq!(settings.stream.websocket_connect_timeout_ms, Some(5678));

    let overrides = apply_stream_timeout_defaults(
        SimpleStreamOptions {
            stream: zedflow_ai::types::StreamOptions {
                timeout_ms: Some(0),
                websocket_connect_timeout_ms: Some(0),
                ..Default::default()
            },
            ..Default::default()
        },
        1234,
        5678,
    );
    assert_eq!(overrides.stream.timeout_ms, Some(0));
    assert_eq!(overrides.stream.websocket_connect_timeout_ms, Some(0));

    assert_eq!(
        apply_stream_timeout_defaults(SimpleStreamOptions::default(), 0, 1)
            .stream
            .timeout_ms,
        Some(i32::MAX as u64)
    );
}
