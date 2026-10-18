//! Fixed diagnostic boundaries, with no clock or allocation in the default sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    ValidationEnergy,
    VadNormalization,
    VadInference,
    StateCreate,
    NativeFull,
    TextJoin,
    StateDrop,
}
impl Phase {
    #[cfg(feature = "measurement")]
    pub(super) const fn index(self) -> usize {
        self as usize
    }
}
