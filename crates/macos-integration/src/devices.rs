//! Read-only CoreAudio device discovery. Never creates an AudioUnit or IOProc.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DeviceInventoryError {
    #[error("CoreAudio property query failed ({0})")]
    Property(i32),
    #[error("CoreAudio returned invalid or excessive device metadata")]
    InvalidMetadata,
}

trait Inventory {
    fn device_ids(&self) -> Result<Vec<u32>, DeviceInventoryError>;
    fn input_stream_count(&self, device: u32) -> Result<usize, DeviceInventoryError>;
    fn name(&self, device: u32) -> Result<String, DeviceInventoryError>;
}

/// Present input-device names, including duplicates. Presence is not a promise
/// that a later stream can be opened. Any failed query invalidates the snapshot.
pub fn input_device_names() -> Result<Vec<String>, DeviceInventoryError> {
    discover(&CoreAudio)
}

fn discover(backend: &impl Inventory) -> Result<Vec<String>, DeviceInventoryError> {
    let ids = backend.device_ids()?;
    if ids.len() > 1024 {
        return Err(DeviceInventoryError::InvalidMetadata);
    }
    let mut names = Vec::new();
    for id in ids {
        if backend.input_stream_count(id)? != 0 {
            if names.len() == 128 {
                return Err(DeviceInventoryError::InvalidMetadata);
            }
            names.push(backend.name(id)?);
        }
    }
    Ok(names)
}

use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};
use std::{ffi::c_void, mem::size_of, ptr};

// Public CoreAudio AudioHardware{,Base}.h property selectors and ABI.
#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}
const GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const INPUT: u32 = u32::from_be_bytes(*b"inpt");
const DEVICES: u32 = u32::from_be_bytes(*b"dev#");
const STREAMS: u32 = u32::from_be_bytes(*b"stm#");
const NAME: u32 = u32::from_be_bytes(*b"lnam");
#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}
fn address(selector: u32, scope: u32) -> Address {
    Address {
        selector,
        scope,
        element: 0,
    }
}
fn check(status: i32) -> Result<(), DeviceInventoryError> {
    if status == 0 {
        Ok(())
    } else {
        Err(DeviceInventoryError::Property(status))
    }
}
fn property_size(object: u32, address: &Address) -> Result<u32, DeviceInventoryError> {
    let mut size = 0;
    // SAFETY: address and size are valid for the synchronous public read-only
    // property query. There is no qualifier and no listener/IO registration.
    check(unsafe { AudioObjectGetPropertyDataSize(object, address, 0, ptr::null(), &mut size) })?;
    Ok(size)
}
fn id_count(bytes: u32) -> Result<usize, DeviceInventoryError> {
    if !bytes.is_multiple_of(4) || bytes / 4 > 1024 {
        return Err(DeviceInventoryError::InvalidMetadata);
    }
    Ok((bytes / 4) as usize)
}
struct CoreAudio;
impl Inventory for CoreAudio {
    fn device_ids(&self) -> Result<Vec<u32>, DeviceInventoryError> {
        let address = address(DEVICES, GLOBAL);
        let mut bytes = property_size(1, &address)?;
        let mut ids = vec![0; id_count(bytes)?];
        if ids.is_empty() {
            return Ok(ids);
        }
        let capacity_bytes = bytes;
        // SAFETY: ids contains initialized u32 storage of exactly bytes length;
        // CoreAudio receives that capacity, and cannot retain the borrowed pointers.
        check(unsafe {
            AudioObjectGetPropertyData(
                1,
                &address,
                0,
                ptr::null(),
                &mut bytes,
                ids.as_mut_ptr().cast(),
            )
        })?;
        if bytes > capacity_bytes {
            return Err(DeviceInventoryError::InvalidMetadata);
        }
        ids.truncate(id_count(bytes)?);
        Ok(ids)
    }
    fn input_stream_count(&self, device: u32) -> Result<usize, DeviceInventoryError> {
        // Input-scope AudioStreamID array size establishes presence without
        // opening an AudioUnit, querying formats or starting any device IO.
        id_count(property_size(device, &address(STREAMS, INPUT))?)
    }
    fn name(&self, device: u32) -> Result<String, DeviceInventoryError> {
        let mut raw: CFStringRef = ptr::null();
        let mut bytes = size_of::<CFStringRef>() as u32;
        // SAFETY: CoreAudio's name property returns one owned CFStringRef into
        // correctly sized pointer storage. AudioHardwareBase.h assigns release
        // responsibility to the caller; wrap_under_create_rule supplies RAII.
        check(unsafe {
            AudioObjectGetPropertyData(
                device,
                &address(NAME, GLOBAL),
                0,
                ptr::null(),
                &mut bytes,
                (&mut raw as *mut CFStringRef).cast(),
            )
        })?;
        if raw.is_null() {
            return Err(DeviceInventoryError::InvalidMetadata);
        }
        // SAFETY: a successful kAudioObjectPropertyName query returns an owned
        // CFString. Ownership is taken even if subsequent metadata checks fail.
        let name = unsafe { CFString::wrap_under_create_rule(raw) };
        if bytes != size_of::<CFStringRef>() as u32 || name.char_len() > 512 {
            return Err(DeviceInventoryError::InvalidMetadata);
        }
        Ok(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake;
    impl Inventory for Fake {
        fn device_ids(&self) -> Result<Vec<u32>, DeviceInventoryError> {
            Ok(vec![1, 2, 3])
        }
        fn input_stream_count(&self, id: u32) -> Result<usize, DeviceInventoryError> {
            Ok(usize::from(id != 2))
        }
        fn name(&self, id: u32) -> Result<String, DeviceInventoryError> {
            assert_ne!(id, 2, "output-only devices must not require a name query");
            Ok("Microphone".into())
        }
    }
    #[test]
    fn input_stream_presence_filters_outputs_and_preserves_duplicate_names()
    -> Result<(), DeviceInventoryError> {
        assert_eq!(discover(&Fake)?, vec!["Microphone", "Microphone"]);
        Ok(())
    }
    struct Fault {
        count: usize,
        failure: u8,
    }
    impl Inventory for Fault {
        fn device_ids(&self) -> Result<Vec<u32>, DeviceInventoryError> {
            if self.failure == 1 {
                return Err(DeviceInventoryError::Property(-10));
            }
            Ok(vec![1; self.count])
        }
        fn input_stream_count(&self, _: u32) -> Result<usize, DeviceInventoryError> {
            if self.failure == 2 {
                return Err(DeviceInventoryError::Property(-20));
            }
            Ok(1)
        }
        fn name(&self, _: u32) -> Result<String, DeviceInventoryError> {
            if self.failure == 3 {
                return Err(DeviceInventoryError::Property(-30));
            }
            Ok("Input".into())
        }
    }
    #[test]
    fn any_property_failure_invalidates_the_inventory() {
        for (failure, code) in [(1, -10), (2, -20), (3, -30)] {
            assert_eq!(
                discover(&Fault { count: 2, failure }),
                Err(DeviceInventoryError::Property(code))
            );
        }
    }
    #[test]
    fn physical_input_count_and_property_sizes_are_bounded() -> Result<(), DeviceInventoryError> {
        assert_eq!(
            discover(&Fault {
                count: 128,
                failure: 0
            })?
            .len(),
            128
        );
        for count in [129, 1025] {
            assert_eq!(
                discover(&Fault { count, failure: 0 }),
                Err(DeviceInventoryError::InvalidMetadata)
            );
        }
        assert_eq!(id_count(0)?, 0);
        assert_eq!(id_count(4096)?, 1024);
        for bytes in [1, 3, 4097, 4100, u32::MAX] {
            assert_eq!(id_count(bytes), Err(DeviceInventoryError::InvalidMetadata));
        }
        Ok(())
    }
}
