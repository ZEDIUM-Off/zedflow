use std::{cell::RefCell, rc::Rc};

use zedflow_coding_agent::{
    extension_editor::{ExtensionEditorComponent, ExternalEditor},
    extension_input::{ExtensionInputComponent, ExtensionInputOptions},
    extension_selector::{ExtensionSelectorComponent, ExtensionSelectorOptions},
    keybindings::KeybindingsManager,
    modes_interactive_components_index::custom_editor::CustomEditor,
};
use zedflow_tui::{Component, set_keybindings};

fn keybindings() -> KeybindingsManager {
    KeybindingsManager::new(Default::default(), None)
}

fn install_keybindings() {
    set_keybindings(keybindings().inner().clone());
}

#[test]
fn custom_editor_applies_extension_and_application_keys_before_editor_keys() {
    install_keybindings();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut editor = CustomEditor::new(keybindings());

    editor.on_extension_shortcut = Some(Box::new({
        let events = Rc::clone(&events);
        move |data| {
            if data == "!" {
                events.borrow_mut().push("extension");
                true
            } else {
                false
            }
        }
    }));
    editor.on_paste_image = Some(Box::new({
        let events = Rc::clone(&events);
        move || events.borrow_mut().push("paste")
    }));
    editor.on_escape = Some(Box::new({
        let events = Rc::clone(&events);
        move || events.borrow_mut().push("escape")
    }));
    editor.on_ctrl_d = Some(Box::new({
        let events = Rc::clone(&events);
        move || events.borrow_mut().push("exit")
    }));
    editor.on_action("app.model.select", {
        let events = Rc::clone(&events);
        move || events.borrow_mut().push("model")
    });

    editor.handle_input("!");
    editor.handle_input("\x16");
    editor.handle_input("\x1b");
    editor.handle_input("\x04");
    editor.handle_input("\x0c");
    assert_eq!(
        *events.borrow(),
        ["extension", "paste", "escape", "exit", "model"]
    );

    editor.editor_mut().set_text("ab");
    editor.handle_input("\x1b[D");
    editor.handle_input("\x04");
    assert_eq!(editor.editor().get_text(), "a");
}

#[test]
fn extension_input_submits_cancels_times_out_and_disposes() {
    install_keybindings();
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let cancelled = Rc::new(RefCell::new(0));
    let mut input = ExtensionInputComponent::new(
        "Question",
        Some("ignored like Pi"),
        {
            let submitted = Rc::clone(&submitted);
            move |value| submitted.borrow_mut().push(value)
        },
        {
            let cancelled = Rc::clone(&cancelled);
            move || *cancelled.borrow_mut() += 1
        },
        ExtensionInputOptions {
            timeout_ms: Some(1_500),
        },
    );

    input.set_focused(true);
    input.handle_input("answer");
    input.handle_input("\r");
    input.handle_input("\x1b");
    assert!(input.is_focused());
    assert_eq!(*submitted.borrow(), ["answer"]);
    assert_eq!(*cancelled.borrow(), 1);
    assert_eq!(input.title(), "Question (2s)");
    input.tick_timeout();
    input.tick_timeout();
    assert_eq!(*cancelled.borrow(), 2);

    let mut disposed = ExtensionInputComponent::new(
        "Question",
        None,
        |_| {},
        {
            let cancelled = Rc::clone(&cancelled);
            move || *cancelled.borrow_mut() += 1
        },
        ExtensionInputOptions {
            timeout_ms: Some(1_000),
        },
    );
    disposed.dispose();
    disposed.tick_timeout();
    assert_eq!(*cancelled.borrow(), 2);
}

#[test]
fn extension_selector_navigates_selects_toggles_cancels_and_times_out() {
    install_keybindings();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let cancelled = Rc::new(RefCell::new(0));
    let toggled = Rc::new(RefCell::new(0));
    let mut selector = ExtensionSelectorComponent::new(
        "Choose",
        vec!["one".into(), "two".into()],
        {
            let selected = Rc::clone(&selected);
            move |value| selected.borrow_mut().push(value)
        },
        {
            let cancelled = Rc::clone(&cancelled);
            move || *cancelled.borrow_mut() += 1
        },
        ExtensionSelectorOptions {
            timeout_ms: Some(1_000),
            on_toggle_tools_expanded: Some(Box::new({
                let toggled = Rc::clone(&toggled);
                move || *toggled.borrow_mut() += 1
            })),
        },
    );

    selector.handle_input("j");
    selector.handle_input("j");
    selector.handle_input("\r");
    selector.handle_input("\x0f");
    selector.handle_input("\x1b");
    assert_eq!(selector.selected_index(), 1);
    assert_eq!(*selected.borrow(), ["two"]);
    assert_eq!(*toggled.borrow(), 1);
    assert_eq!(*cancelled.borrow(), 1);
    selector.tick_timeout();
    assert_eq!(*cancelled.borrow(), 2);
}

#[test]
fn extension_editor_uses_only_the_injected_external_editor_and_keeps_cancel_submit() {
    install_keybindings();
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let cancelled = Rc::new(RefCell::new(0));
    let external_inputs = Rc::new(RefCell::new(Vec::new()));
    let external: ExternalEditor = Box::new({
        let external_inputs = Rc::clone(&external_inputs);
        move |text| {
            external_inputs.borrow_mut().push(text.to_owned());
            Ok(Some("edited\n".into()))
        }
    });
    let mut editor = ExtensionEditorComponent::new(
        keybindings(),
        "Edit",
        Some("prefill"),
        {
            let submitted = Rc::clone(&submitted);
            move |value| submitted.borrow_mut().push(value)
        },
        {
            let cancelled = Rc::clone(&cancelled);
            move || *cancelled.borrow_mut() += 1
        },
        Some(external),
    );

    editor.set_focused(true);
    editor.handle_input("\x07");
    assert_eq!(*external_inputs.borrow(), ["prefill"]);
    assert_eq!(editor.text(), "edited");
    editor.handle_input("\r");
    editor.handle_input("\x1b");
    assert_eq!(*submitted.borrow(), ["edited"]);
    assert_eq!(*cancelled.borrow(), 1);
    assert!(editor.is_focused());
}
