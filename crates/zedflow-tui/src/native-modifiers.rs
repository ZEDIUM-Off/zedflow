//! Native modifier probing. The Pi helper is optional; unsupported hosts report false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    Shift,
    Command,
    Control,
    Option,
}
pub fn is_native_modifier_pressed(_key: ModifierKey) -> bool {
    false
}
