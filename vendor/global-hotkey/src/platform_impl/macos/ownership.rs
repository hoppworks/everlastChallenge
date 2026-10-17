use std::collections::BTreeMap;

// Keep the OS registration owned until the fallible platform operation succeeds.
// This seam also allows the failure/retry contract to be tested without global keys.
pub(super) fn unregister_owned<T, E>(
    registrations: &mut BTreeMap<u32, T>,
    id: u32,
    unregister: impl FnOnce(&T) -> Result<(), E>,
) -> Result<(), E> {
    if let Some(registration) = registrations.get(&id) {
        unregister(registration)?;
        registrations.remove(&id);
    }
    Ok(())
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn failed_unregister_retains_handle_and_retry_releases_it() {
        let mut registrations = BTreeMap::from([(7, 42)]);
        let failed = unregister_owned(&mut registrations, 7, |handle| {
            assert_eq!(*handle, 42);
            Err("controlled removal failure")
        });
        assert!(failed.is_err());
        assert_eq!(registrations.get(&7), Some(&42));
        let mut retried = false;
        unregister_owned(&mut registrations, 7, |handle| {
            assert_eq!(*handle, 42);
            retried = true;
            Ok::<_, &str>(())
        })
        .unwrap();
        assert!(retried);
        assert!(registrations.is_empty());
    }
}
