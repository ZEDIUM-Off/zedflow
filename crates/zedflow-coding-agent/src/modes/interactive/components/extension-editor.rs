//! Multi-line extension editor with injected external-editor behavior.

use crate::keybindings::KeybindingsManager;
use zedflow_tui::{Component, Editor, Focusable, get_keybindings};

pub type ExternalEditor = Box<dyn FnMut(&str) -> Result<Option<String>, String>>;

pub struct ExtensionEditorComponent {
    title: String,
    editor: Editor,
    keybindings: KeybindingsManager,
    on_cancel: Box<dyn FnMut()>,
    external_editor: Option<ExternalEditor>,
    external_editor_error: Option<String>,
    focused: bool,
}

impl ExtensionEditorComponent {
    pub fn new(
        keybindings: KeybindingsManager,
        title: impl Into<String>,
        prefill: Option<&str>,
        on_submit: impl FnMut(String) + 'static,
        on_cancel: impl FnMut() + 'static,
        external_editor: Option<ExternalEditor>,
    ) -> Self {
        let mut editor = Editor::new();
        if let Some(prefill) = prefill.filter(|text| !text.is_empty()) {
            editor.set_text(prefill);
        }
        let mut on_submit = on_submit;
        editor.on_submit = Some(Box::new(move |text| on_submit(text.to_owned())));
        Self {
            title: title.into(),
            editor,
            keybindings,
            on_cancel: Box::new(on_cancel),
            external_editor,
            external_editor_error: None,
            focused: false,
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.editor.get_text()
    }

    #[must_use]
    pub fn external_editor_error(&self) -> Option<&str> {
        self.external_editor_error.as_deref()
    }

    fn open_external_editor(&mut self) {
        let Some(edit) = &mut self.external_editor else {
            return;
        };
        self.external_editor_error = None;
        match edit(&self.editor.get_text()) {
            Ok(Some(content)) => self
                .editor
                .set_text(content.strip_suffix('\n').unwrap_or(&content)),
            Ok(None) => {}
            Err(error) => self.external_editor_error = Some(error),
        }
    }
}

impl Component for ExtensionEditorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![
            "─".repeat(width.max(1)),
            String::new(),
            format!(" {}", self.title),
            String::new(),
        ];
        lines.extend(self.editor.render(width));
        lines.extend([
            String::new(),
            if self.external_editor.is_some() {
                " enter submit  shift+enter newline  escape cancel  ctrl+g external editor".into()
            } else {
                " enter submit  shift+enter newline  escape cancel".into()
            },
            String::new(),
            "─".repeat(width.max(1)),
        ]);
        lines
    }

    fn handle_input(&mut self, data: &str) {
        if get_keybindings()
            .lock()
            .unwrap()
            .matches(data, "tui.select.cancel")
        {
            (self.on_cancel)();
        } else if self.keybindings.matches(data, "app.editor.external") {
            self.open_external_editor();
        } else {
            self.editor.handle_input(data);
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        Focusable::set_focused(&mut self.editor, focused);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}
