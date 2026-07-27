use zedflow_tui::utils::visible_width;

struct VirtualTerminal {
    width: usize,
    lines: Vec<String>,
}
impl VirtualTerminal {
    fn new(width: usize) -> Self {
        Self {
            width,
            lines: Vec::new(),
        }
    }
    fn write(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }
    fn viewport(&self) -> &[String] {
        &self.lines
    }
}

#[test]
fn records_lines_without_counting_ansi_columns() {
    let mut terminal = VirtualTerminal::new(10);
    terminal.write("\x1b[31mhello\x1b[0m");
    assert_eq!(terminal.width, 10);
    assert_eq!(visible_width(&terminal.viewport()[0]), 5);
}
