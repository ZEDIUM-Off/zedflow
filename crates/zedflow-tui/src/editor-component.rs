pub trait EditorComponent: crate::Component {
    fn get_text(&self) -> String;
    fn set_text(&mut self, text: &str);
}
