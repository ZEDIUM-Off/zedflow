//! User-message picker used by fork and branch commands.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessageItem {
    pub id: String,
    pub text: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMessageSelectorAction {
    None,
    Select(String),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct UserMessageSelectorState {
    messages: Vec<UserMessageItem>,
    selected: usize,
}

impl UserMessageSelectorState {
    #[must_use]
    pub fn new(messages: Vec<UserMessageItem>, initial_selected_id: Option<&str>) -> Self {
        let selected = initial_selected_id
            .and_then(|id| messages.iter().position(|message| message.id == id))
            .unwrap_or_else(|| messages.len().saturating_sub(1));
        Self { messages, selected }
    }

    #[must_use]
    pub fn messages(&self) -> &[UserMessageItem] {
        &self.messages
    }

    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn move_up(&mut self) {
        if !self.messages.is_empty() {
            self.selected = if self.selected == 0 {
                self.messages.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.messages.is_empty() {
            self.selected = (self.selected + 1) % self.messages.len();
        }
    }

    #[must_use]
    pub fn select(&self) -> UserMessageSelectorAction {
        self.messages
            .get(self.selected)
            .map_or(UserMessageSelectorAction::None, |message| {
                UserMessageSelectorAction::Select(message.id.clone())
            })
    }

    #[must_use]
    pub const fn cancel(&self) -> UserMessageSelectorAction {
        UserMessageSelectorAction::Cancel
    }

    #[must_use]
    pub fn visible_range(&self, max_visible: usize) -> std::ops::Range<usize> {
        if self.messages.is_empty() || max_visible == 0 {
            return 0..0;
        }
        let start = self
            .selected
            .saturating_sub(max_visible / 2)
            .min(self.messages.len().saturating_sub(max_visible));
        start..(start + max_visible).min(self.messages.len())
    }

    #[must_use]
    pub fn normalized_text(message: &UserMessageItem) -> String {
        message.text.replace('\n', " ").trim().to_owned()
    }
}
