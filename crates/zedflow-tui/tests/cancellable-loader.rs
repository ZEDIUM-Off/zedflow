use std::{cell::Cell, collections::HashMap, rc::Rc};

use zedflow_tui::{
    CancellableLoader, Component, KeybindingsManager, set_keybindings, tui_keybindings,
};

#[test]
fn cancellable_loader_signals_and_calls_abort_once() {
    let mut user = HashMap::new();
    user.insert("tui.select.cancel".into(), vec!["ctrl+x".into()]);
    set_keybindings(KeybindingsManager::new(tui_keybindings(), user));

    let calls = Rc::new(Cell::new(0));
    let callback_calls = calls.clone();
    let mut loader = CancellableLoader::new("Working...");
    let signal = loader.signal();
    loader.on_abort = Some(Box::new(move || {
        callback_calls.set(callback_calls.get() + 1)
    }));

    loader.handle_input("\x1b");
    assert!(!signal.aborted());
    loader.handle_input("\x18");
    loader.handle_input("\x18");
    assert!(loader.aborted());
    assert!(signal.aborted());
    assert_eq!(calls.get(), 1);
    loader.dispose();

    set_keybindings(KeybindingsManager::new(tui_keybindings(), HashMap::new()));
}
