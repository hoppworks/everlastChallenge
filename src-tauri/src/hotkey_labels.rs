//! Pure projection of a complete native layout snapshot. Never changes a binding.
use crate::{
    contracts::KeyLabel,
    settings::{Hotkey, PhysicalKey},
};
use macos_integration::keyboard::KeyLabels;
pub const KEYS: [PhysicalKey; 26] = [
    PhysicalKey::A,
    PhysicalKey::B,
    PhysicalKey::C,
    PhysicalKey::D,
    PhysicalKey::E,
    PhysicalKey::F,
    PhysicalKey::G,
    PhysicalKey::H,
    PhysicalKey::I,
    PhysicalKey::J,
    PhysicalKey::K,
    PhysicalKey::L,
    PhysicalKey::M,
    PhysicalKey::N,
    PhysicalKey::O,
    PhysicalKey::P,
    PhysicalKey::Q,
    PhysicalKey::R,
    PhysicalKey::S,
    PhysicalKey::T,
    PhysicalKey::U,
    PhysicalKey::V,
    PhysicalKey::W,
    PhysicalKey::X,
    PhysicalKey::Y,
    PhysicalKey::Z,
];
pub fn options(labels: &KeyLabels) -> Vec<KeyLabel> {
    KEYS.into_iter()
        .enumerate()
        .map(|(i, key)| KeyLabel {
            key,
            plain: labels.plain[i].clone(),
            command: labels.command[i].clone(),
        })
        .collect()
}
pub fn shortcut(key: &Hotkey, labels: &KeyLabels) -> String {
    let Some(i) = KEYS.iter().position(|candidate| *candidate == key.key) else {
        return format!("[physisch {:?}; Layout unbekannt]", key.key);
    };
    let label = if key.command {
        &labels.command[i]
    } else {
        &labels.plain[i]
    };
    format!(
        "{}{}{}{}{}",
        if key.control { "⌃" } else { "" },
        if key.option { "⌥" } else { "" },
        if key.shift { "⇧" } else { "" },
        if key.command { "⌘" } else { "" },
        label
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_selection_uses_full_layout_snapshot_and_preserves_physical_settings() {
        let labels = KeyLabels::from_translator(|code, command| {
            Ok(match (code, command) {
                (0x02, false) => "E",
                (0x02, true) => "D",
                (0x00, _) => "A",
                _ => "X",
            }
            .into())
        });
        let mut key = crate::settings::Settings::default().hotkey;
        assert_eq!(shortcut(&key, &labels), "⌥E");
        key.command = true;
        assert_eq!(shortcut(&key, &labels), "⌥⌘D");
        key.key = PhysicalKey::A;
        assert_eq!(shortcut(&key, &labels), "⌥⌘A");
        assert_eq!(key.key, PhysicalKey::A);
        assert_eq!(options(&labels).len(), 26);
        assert_eq!(options(&labels)[3].key, PhysicalKey::D);
    }
}
