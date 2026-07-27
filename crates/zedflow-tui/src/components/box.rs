use crate::Component;
pub struct Box {
    pub children: Vec<std::boxed::Box<dyn Component>>,
    pub padding_x: usize,
    pub padding_y: usize,
}
impl Box {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            children: vec![],
            padding_x: x,
            padding_y: y,
        }
    }
    pub fn add_child(&mut self, c: impl Component + 'static) {
        self.children.push(std::boxed::Box::new(c))
    }
    pub fn clear(&mut self) {
        self.children.clear()
    }
}
impl Component for Box {
    fn render(&self, w: usize) -> Vec<String> {
        if self.children.is_empty() {
            return vec![];
        }
        let cw = w.saturating_sub(self.padding_x * 2);
        let mut o = vec![" ".repeat(w); self.padding_y]
            .into_iter()
            .collect::<Vec<_>>();
        for c in &self.children {
            for l in c.render(cw) {
                let s = format!("{}{}", " ".repeat(self.padding_x), l);
                o.push(format!(
                    "{}{}",
                    s,
                    " ".repeat(w.saturating_sub(crate::utils::visible_width(&s)))
                ));
            }
        }
        o.extend(std::iter::repeat_n(" ".repeat(w), self.padding_y));
        o
    }
}
