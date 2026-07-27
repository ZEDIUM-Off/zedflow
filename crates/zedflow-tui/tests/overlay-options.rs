use zedflow_tui::{Component, OverlayAnchor, OverlayMargin, OverlayOptions, SizeValue, Tui};

struct Line(&'static str);
impl Component for Line {
    fn render(&self, _: usize) -> Vec<String> {
        vec![self.0.into()]
    }
}

#[test]
fn overlay_width_position_and_visibility_are_applied() {
    let mut tui = Tui::new();
    tui.show_overlay_with_options(
        Line("overlay text beyond width"),
        OverlayOptions {
            width: Some(SizeValue::Cells(7)),
            anchor: OverlayAnchor::BottomRight,
            margin: OverlayMargin {
                right: 1,
                bottom: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    );

    let frame = tui.render_frame(20, 6);
    assert_eq!(frame.len(), 6);
    assert!(frame[4].contains("overlay"));
    assert_eq!(zedflow_tui::visible_width(&frame[4]), 20);
}

#[test]
fn percentage_sizes_and_visibility_are_resolved_each_frame() {
    let mut tui = Tui::new();
    tui.show_overlay_with_options(
        Line("visible"),
        OverlayOptions {
            width: Some(SizeValue::Percent(50.0)),
            visible: Some(Box::new(|width, _| width >= 40)),
            ..Default::default()
        },
    );
    assert_eq!(tui.render_frame(20, 4), Vec::<String>::new());
    assert!(
        tui.render_frame(40, 4)
            .iter()
            .any(|line| line.contains("visible"))
    );
}
