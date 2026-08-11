//! Application keybindings layered over Pi's reusable editor.

use crate::keybindings::KeybindingsManager;
use zedflow_tui::{Component, Editor, Focusable};

type Handler = Box<dyn FnMut()>;

pub struct CustomEditor {
    editor: Editor,
    keybindings: KeybindingsManager,
    action_handlers: Vec<(String, Handler)>,
    pub on_escape: Option<Handler>,
    pub on_ctrl_d: Option<Handler>,
    pub on_paste_image: Option<Handler>,
    pub on_extension_shortcut: Option<Box<dyn FnMut(&str) -> bool>>,
}

impl CustomEditor {
    #[must_use]
    pub fn new(keybindings: KeybindingsManager) -> Self {
        Self {
            editor: Editor::new(),
            keybindings,
            action_handlers: Vec::new(),
            on_escape: None,
            on_ctrl_d: None,
            on_paste_image: None,
            on_extension_shortcut: None,
        }
    }

    pub fn on_action(&mut self, action: impl Into<String>, handler: impl FnMut() + 'static) {
        let action = action.into();
        if let Some((_, current)) = self
            .action_handlers
            .iter_mut()
            .find(|(registered, _)| registered == &action)
        {
            *current = Box::new(handler);
        } else {
            self.action_handlers.push((action, Box::new(handler)));
        }
    }

    #[must_use]
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }
}

impl Component for CustomEditor {
    fn render(&self, width: usize) -> Vec<String> {
        self.editor.render(width)
    }

    fn invalidate(&mut self) {
        self.editor.invalidate();
    }

    fn handle_input(&mut self, data: &str) {
        if self
            .on_extension_shortcut
            .as_mut()
            .is_some_and(|handler| handler(data))
        {
            return;
        }
        if self.keybindings.matches(data, "app.clipboard.pasteImage") {
            if let Some(handler) = &mut self.on_paste_image {
                handler();
            }
            return;
        }
        if self.keybindings.matches(data, "app.interrupt") {
            if !self.editor.is_showing_autocomplete() {
                let handler = self.on_escape.as_mut().or_else(|| {
                    self.action_handlers
                        .iter_mut()
                        .find(|(action, _)| action == "app.interrupt")
                        .map(|(_, handler)| handler)
                });
                if let Some(handler) = handler {
                    handler();
                    return;
                }
            }
            self.editor.handle_input(data);
            return;
        }
        if self.keybindings.matches(data, "app.exit") {
            if self.editor.get_text().is_empty() {
                let handler = self.on_ctrl_d.as_mut().or_else(|| {
                    self.action_handlers
                        .iter_mut()
                        .find(|(action, _)| action == "app.exit")
                        .map(|(_, handler)| handler)
                });
                if let Some(handler) = handler {
                    handler();
                }
                return;
            }
        }
        if let Some((_, handler)) = self.action_handlers.iter_mut().find(|(action, _)| {
            action != "app.interrupt"
                && action != "app.exit"
                && self.keybindings.matches(data, action)
        }) {
            handler();
            return;
        }
        self.editor.handle_input(data);
    }

    fn set_focused(&mut self, focused: bool) {
        Focusable::set_focused(&mut self.editor, focused);
    }

    fn is_focused(&self) -> bool {
        Focusable::is_focused(&self.editor)
    }
}
