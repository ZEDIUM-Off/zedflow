#[derive(Clone, Debug)]
pub struct AutocompleteItem {
    pub label: String,
    pub value: String,
}
pub type AutocompleteSuggestions = Vec<AutocompleteItem>;
pub trait AutocompleteProvider {
    fn suggestions(&self, prefix: &str) -> AutocompleteSuggestions;
}
#[derive(Clone, Debug)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}
pub struct CombinedAutocompleteProvider;
