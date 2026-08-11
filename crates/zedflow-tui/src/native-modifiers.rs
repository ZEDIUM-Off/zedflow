//! Native modifier probing. Unsupported hosts report false.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    Shift,
    Command,
    Control,
    Option,
}

#[cfg(target_os = "macos")]
pub fn is_native_modifier_pressed(key: ModifierKey) -> bool {
    use objc2_core_graphics::{CGEventFlags, CGEventSource, CGEventSourceStateID};

    let flag = match key {
        ModifierKey::Shift => CGEventFlags::MaskShift,
        ModifierKey::Command => CGEventFlags::MaskCommand,
        ModifierKey::Control => CGEventFlags::MaskControl,
        ModifierKey::Option => CGEventFlags::MaskAlternate,
    };
    CGEventSource::flags_state(CGEventSourceStateID::HIDSystemState).contains(flag)
}

#[cfg(not(target_os = "macos"))]
pub fn is_native_modifier_pressed(_key: ModifierKey) -> bool {
    false
}
