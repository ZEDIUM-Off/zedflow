use zedflow_tui::terminal_image::is_image_line;

#[test]
fn detects_image_protocol_after_a_text_prefix() {
    let line = format!(
        "Read image file [image/jpeg] \x1b]1337;File=inline=1:{}",
        "A".repeat(300_000)
    );
    assert!(is_image_line(&line));
    assert!(is_image_line("text before \x1b_Ga=T,f=100;AAAA\x1b\\"));
}
