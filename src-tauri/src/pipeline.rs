//! Internal immutable pipeline selection. App starts retain the reference path.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    #[default]
    Reference,
    #[cfg(any(test, feature = "measurement"))]
    FullPrefix,
}

/// Kept inactive until the complete candidate qualification is accepted.
pub(crate) const APP_POLICY: Policy = Policy::Reference;

impl Policy {
    pub(crate) fn is_full_prefix(self) -> bool {
        #[cfg(any(test, feature = "measurement"))]
        {
            self == Self::FullPrefix
        }
        #[cfg(not(any(test, feature = "measurement")))]
        {
            false
        }
    }
}
