use crate::autocomplete::AutocompleteProvider;

/// Common surface implemented by built-in and extension editors.
pub trait EditorComponent: crate::Component {
    fn get_text(&self) -> String;
    fn set_text(&mut self, text: &str);
    fn handle_input(&mut self, data: &str);
    fn add_to_history(&mut self, _text: &str) {}
    fn insert_text_at_cursor(&mut self, _text: &str) {}
    fn get_expanded_text(&self) -> String {
        self.get_text()
    }
    fn set_autocomplete_provider(&mut self, _provider: Box<dyn AutocompleteProvider>) {}
    fn set_padding_x(&mut self, _padding: usize) {}
    fn set_autocomplete_max_visible(&mut self, _max_visible: usize) {}
}
