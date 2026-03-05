use std::fmt::Debug;

mod macros;
pub(crate) use macros::*;

/// Mask for reference count. It's also the maximum reference count (RC_MAX) we
/// can have, since the last bit is used for borrow flags.
pub const RC_MASK: usize = !0b1;
/// One unit of reference count
pub const RC_UNIT: usize = 0b10;
/// Mask for extracting borrowed bits
pub const BORROW_MASK: usize = 0b1;

/// Snapshot of the current state.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Snapshot(pub usize);

impl Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("count", &self.count())
            .field("borrowed", &self.is_borrowed())
            .finish()
    }
}

impl Snapshot {
    /// Current reference count.
    pub fn count(&self) -> usize {
        (self.0 & RC_MASK) >> 1
    }

    pub fn is_shared(&self) -> bool {
        self.count() > 1
    }

    pub fn is_borrowed(&self) -> bool {
        (self.0 & BORROW_MASK) != 0
    }
}

impl From<usize> for Snapshot {
    fn from(value: usize) -> Self {
        Snapshot(value)
    }
}
