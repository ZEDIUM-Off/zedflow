use crate::Component;
pub struct Image {
    pub data: Vec<u8>,
    pub alt: String,
}
impl Image {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            alt: String::new(),
        }
    }
}
impl Component for Image {
    fn render(&self, _: usize) -> Vec<String> {
        if self.data.is_empty() {
            vec![]
        } else {
            vec![self.alt.clone()]
        }
    }
}
