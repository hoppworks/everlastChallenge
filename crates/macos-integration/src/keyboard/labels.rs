//! Layout labels for physical ANSI letters; modifiers never synthesize input.
use super::*;

/// Carbon Events.h kVK_ANSI_* values, in A–Z order. Identical to the pinned
/// global-hotkey macOS registration mapping; these are positions, not characters.
pub const LETTER_CODES: [u16; 26] = [
    0x00, 0x0b, 0x08, 0x02, 0x0e, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2e, 0x2d, 0x1f, 0x23,
    0x0c, 0x0f, 0x01, 0x11, 0x20, 0x09, 0x0d, 0x07, 0x10, 0x06,
];
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLabels {
    pub plain: [String; 26],
    pub command: [String; 26],
}
impl Default for KeyLabels {
    fn default() -> Self {
        let fallback = std::array::from_fn(|i| {
            format!(
                "[physisch {}; Layout unbekannt]",
                char::from(b'A' + i as u8)
            )
        });
        Self {
            plain: fallback.clone(),
            command: fallback,
        }
    }
}
impl KeyLabels {
    /// Resolve both keycap layers in one bounded layout snapshot. The translator
    /// receives physical Carbon codes and only the Command layer, never Option/
    /// Control text-input modifiers. Errors are explicit per-position fallbacks.
    pub fn from_translator(
        mut translate: impl FnMut(u16, bool) -> Result<String, KeyboardError>,
    ) -> Self {
        let mut result = Self::default();
        for (i, code) in LETTER_CODES.into_iter().enumerate() {
            for (command, values) in [(false, &mut result.plain), (true, &mut result.command)] {
                if let Ok(label) = translate(code, command)
                    && !label.is_empty()
                    && label.len() <= 64
                    && !label.chars().any(|c| c.is_control() || c.is_whitespace())
                {
                    values[i] = if label.is_ascii() {
                        label.to_ascii_uppercase()
                    } else {
                        label
                    };
                }
            }
        }
        result
    }
    pub fn read_current() -> Result<Self, super::ObserverError> {
        Ok(Self::current(
            MainThreadMarker::new().ok_or(super::ObserverError::MainThread)?,
        ))
    }
    /// Read one immutable current-layout snapshot on the native main thread.
    /// No fallback to another input source and no input-source selection occurs.
    pub fn current(main: MainThreadMarker) -> Self {
        let Ok(layout) = Layout::current(main) else {
            return Self::default();
        };
        Self::from_translator(|code, command| {
            let mut state = 0;
            let mut length = 0;
            let mut output = [0_u16; 32];
            // SAFETY: retained OS uchr data, bounded writable UTF-16 output;
            // CarbonCore UnicodeUtilities.h: Display=3, Command=(cmdKey>>8)=1,
            // NoDeadKeys=1. Other shortcut modifiers are separate glyphs.
            let status = unsafe {
                UCKeyTranslate(
                    layout.data.bytes().as_ptr().cast(),
                    code,
                    3,
                    u32::from(command),
                    layout.keyboard_type,
                    1,
                    &mut state,
                    output.len() as libc::c_ulong,
                    &mut length,
                    output.as_mut_ptr(),
                )
            };
            if status != 0 || length == 0 || length as usize > output.len() {
                return Err(KeyboardError::Unsupported);
            }
            String::from_utf16(&output[..length as usize]).map_err(|_| KeyboardError::Unsupported)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_position_and_command_layer_are_resolved_without_option_text() {
        let labels = KeyLabels::from_translator(|code, command| {
            if code == 0x02 {
                Ok(if command { "D" } else { "E" }.into())
            } else {
                Err(KeyboardError::Unsupported)
            }
        });
        assert_eq!(labels.plain[3], "E");
        assert_eq!(labels.command[3], "D");
        assert_eq!(labels.plain[0], "[physisch A; Layout unbekannt]");
    }
    #[test]
    fn missing_or_invalid_layout_output_never_reuses_a_previous_label() {
        let valid = KeyLabels::from_translator(|_, _| Ok("a".into()));
        assert_eq!(valid.plain[0], "A");
        let invalid = KeyLabels::from_translator(|code, _| match code {
            0x00 => Ok(String::new()),
            0x02 => Ok("\n".into()),
            0x0c => Ok("x".repeat(65)),
            0x06 => Ok(" ".into()),
            _ => Err(KeyboardError::Unsupported),
        });
        assert_eq!(invalid, KeyLabels::default());
        assert_eq!(invalid.command[3], "[physisch D; Layout unbekannt]");
        // SDK Events.h literals, independent of the registration enum ordering.
        assert_eq!(
            (
                LETTER_CODES[0],
                LETTER_CODES[3],
                LETTER_CODES[16],
                LETTER_CODES[25]
            ),
            (0x00, 0x02, 0x0c, 0x06)
        );
    }
}
