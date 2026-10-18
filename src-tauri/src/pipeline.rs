//! Internal immutable pipeline selection. App starts retain the reference path.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    #[default]
    Reference,
    FullPrefix,
}

/// Kept inactive until the complete candidate qualification is accepted.
pub(crate) const APP_POLICY: Policy = Policy::Reference;
