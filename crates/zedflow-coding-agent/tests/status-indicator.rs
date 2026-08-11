use zedflow_coding_agent::status_indicator::{
    StatusIndicator, StatusIndicatorKind, WorkingIndicatorOptions,
};
use zedflow_tui::Component;
#[test]
fn status_indicator_uses_custom_frames_and_safe_interval() {
    let mut status = StatusIndicator::new(
        StatusIndicatorKind::Working,
        "working",
        Some(WorkingIndicatorOptions {
            frames: Some(vec!["a".into(), "b".into()]),
            interval_ms: Some(0),
        }),
    );
    assert_eq!(status.interval_ms(), 80);
    assert!(status.render(20)[1].contains("a working"));
    status.advance_frame();
    assert!(status.render(20)[1].contains("b working"));
}
