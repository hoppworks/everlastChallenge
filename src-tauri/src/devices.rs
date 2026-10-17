//! Bounded device-name discovery. Enumerating devices never opens an input stream.

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InputDevice {
    pub name: String,
    /// False denotes a present name shared by multiple physical devices.
    /// Disconnected names are omitted from the inventory.
    pub available: bool,
}

/// Read-only enumeration; device presence is a snapshot, not a stream-open test.
pub fn input_devices() -> Result<Vec<InputDevice>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|_| "Input devices could not be enumerated".to_owned())?;
    normalize_input_devices(devices.map(|device| {
        device
            .name()
            .map_err(|_| "Input device name could not be read".to_owned())
    }))
}

/// Validate and aggregate a complete refresh, preserving exact hardware names.
/// Ambiguous names cannot identify one device. Invalid entries fail the entire
/// refresh; no partial list or silently renamed selection is returned.
pub fn normalize_input_devices(
    names: impl IntoIterator<Item = Result<String, String>>,
) -> Result<Vec<InputDevice>, String> {
    let mut unique = BTreeMap::new();
    for (index, name) in names.into_iter().enumerate() {
        if index >= 128 {
            return Err("Input device count exceeds 128".into());
        }
        let name = name.map_err(|_| "Input device name could not be read".to_owned())?;
        if name.is_empty()
            || name.len() > 512
            || name.trim() != name
            || name.chars().any(char::is_control)
        {
            return Err("Input device name is invalid".into());
        }
        unique
            .entry(name)
            .and_modify(|available| *available = false)
            .or_insert(true);
    }
    Ok(unique
        .into_iter()
        .map(|(name, available)| InputDevice { name, available })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sorted_and_duplicates_are_visible_but_unavailable() {
        let devices = normalize_input_devices([
            Ok("USB Mikrofon".into()),
            Ok("Built-in".into()),
            Ok("USB Mikrofon".into()),
        ]);
        assert_eq!(
            devices,
            Ok(vec![
                InputDevice {
                    name: "Built-in".into(),
                    available: true
                },
                InputDevice {
                    name: "USB Mikrofon".into(),
                    available: false
                },
            ])
        );
    }

    #[test]
    fn unreadable_or_invalid_names_invalidate_the_entire_refresh() {
        for invalid in [
            "",
            " ",
            " leading",
            "trailing ",
            "line\nbreak",
            "tab\tname",
            "nul\0name",
        ] {
            assert!(normalize_input_devices([Ok("Valid".into()), Ok(invalid.into())]).is_err());
        }
        assert!(normalize_input_devices([Ok("Valid".into()), Err("driver error".into())]).is_err());
    }

    #[test]
    fn byte_and_physical_entry_limits_include_multibyte_and_duplicate_names() {
        let name = "ü".repeat(256);
        assert_eq!(
            normalize_input_devices([Ok(name.clone())]),
            Ok(vec![InputDevice {
                name: name.clone(),
                available: true
            }])
        );
        assert!(normalize_input_devices([Ok(format!("{name}x"))]).is_err());
        let accepted = normalize_input_devices((0..128).map(|_| Ok("Same name".into())));
        assert_eq!(
            accepted,
            Ok(vec![InputDevice {
                name: "Same name".into(),
                available: false
            }])
        );
        assert!(normalize_input_devices((0..129).map(|_| Ok("Same name".into()))).is_err());
        assert_eq!(normalize_input_devices([]), Ok(Vec::new()));
    }

    #[test]
    fn device_contract_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<InputDevice>(r#"{"name":"Mic","available":true,"secret":1}"#)
                .is_err()
        );
    }
}
