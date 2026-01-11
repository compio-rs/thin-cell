use std::{fmt::Debug, path::Display};

/// Encapsulates the bitwise logic for Reference Counting and borrow flags.
///
/// All bits except last are used for Reference Count (RC), while last bit is
/// used for borrow flags (Borrowed).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct State(usize);

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("count", &self.count())
            .field("borrowed", &self.is_borrowed())
            .finish()
    }
}

#[rustfmt::skip]
impl State {
    /// Mask for reference count
    const RC_MASK: usize = !0b1;
    /// One unit of reference count
    const RC_UNIT: usize = 0b10;
    /// Max number of reference count
    const RC_MAX: usize = Self::RC_MASK;
    /// Mask for extracting borrowed bits
    const BORROW_MASK: usize = 0b1;
}

impl State {
    #[inline]
    pub fn new() -> Self {
        // Starts with 1 owner, 0 borrows
        State(Self::RC_UNIT)
    }

    /// Current reference count.
    #[inline]
    pub fn count(self) -> usize {
        (self.0 & Self::RC_MASK) >> 1
    }

    #[inline]
    pub fn is_shared(self) -> bool {
        self.count() > 1
    }

    #[inline]
    pub fn is_borrowed(self) -> bool {
        (self.0 & Self::BORROW_MASK) != 0
    }

    #[inline]
    pub fn inc(self) -> Option<Self> {
        // Check for overflow in the high bits
        if (self.0 & Self::RC_MASK) == Self::RC_MAX {
            None
        } else {
            Some(State(self.0 + Self::RC_UNIT))
        }
    }

    #[inline]
    pub fn dec(self) -> Self {
        State(self.0 - Self::RC_UNIT)
    }

    #[inline]
    pub fn borrow(self) -> Self {
        self.try_borrow().expect("Already borrowed")
    }

    #[inline]
    pub fn try_borrow(self) -> Option<Self> {
        if self.is_borrowed() {
            None
        } else {
            Some(State(self.0 + 1))
        }
    }

    #[inline]
    pub fn unborrow(self) -> Self {
        // Keep RC bits, clear Borrow bits
        State(self.0 & Self::RC_MASK)
    }
}

#[test]
fn test_state_new() {
    let state = State::new();
    assert_eq!(state.count(), 1);
    assert!(!state.is_borrowed());
    assert!(!state.is_shared());
}

#[test]
fn test_state_count() {
    let state = State::new();
    assert_eq!(state.count(), 1);

    let state2 = state.inc().unwrap();
    assert_eq!(state2.count(), 2);

    let state3 = state2.inc().unwrap();
    assert_eq!(state3.count(), 3);
}

#[test]
fn test_state_inc() {
    let mut state = State::new();
    assert_eq!(state.count(), 1);

    for i in 2..=10 {
        state = state.inc().expect("increment should succeed");
        assert_eq!(state.count(), i);
    }
}

#[test]
fn test_state_dec() {
    let state = State::new().inc().unwrap().inc().unwrap().inc().unwrap(); // count = 4

    let state = state.dec(); // count = 3
    assert_eq!(state.count(), 3);

    let state = state.dec(); // count = 2
    assert_eq!(state.count(), 2);

    let state = state.dec(); // count = 1
    assert_eq!(state.count(), 1);
}

#[test]
fn test_state_is_shared() {
    let state = State::new();
    assert!(!state.is_shared());

    let state = state.inc().unwrap();
    assert!(state.is_shared());

    let state = state.inc().unwrap();
    assert!(state.is_shared());

    let state = state.dec();
    assert!(state.is_shared());

    let state = state.dec();
    assert!(!state.is_shared());
}

#[test]
fn test_state_borrow() {
    let state = State::new();
    assert!(!state.is_borrowed());

    let borrowed = state.borrow();
    assert!(borrowed.is_borrowed());
    assert_eq!(borrowed.count(), 1); // RC unchanged
}

#[test]
fn test_state_try_borrow_success() {
    let state = State::new();
    let borrowed = state.try_borrow();
    assert!(borrowed.is_some());

    let borrowed = borrowed.unwrap();
    assert!(borrowed.is_borrowed());
    assert_eq!(borrowed.count(), 1);
}

#[test]
fn test_state_try_borrow_failure() {
    let state = State::new();
    let borrowed = state.borrow();

    // Already borrowed, should fail
    let result = borrowed.try_borrow();
    assert!(result.is_none());
}

#[test]
fn test_state_borrow_panic() {
    // Test that try_borrow returns None when already borrowed
    let state = State::new();
    let borrowed = state.borrow();

    // Should return None since already borrowed
    assert!(borrowed.try_borrow().is_none());
}

#[test]
fn test_state_unborrow() {
    let state = State::new();
    let borrowed = state.borrow();
    assert!(borrowed.is_borrowed());

    let unborrowed = borrowed.unborrow();
    assert!(!unborrowed.is_borrowed());
    assert_eq!(unborrowed.count(), state.count()); // RC unchanged
}

#[test]
fn test_state_borrow_with_multiple_refs() {
    let state = State::new().inc().unwrap().inc().unwrap(); // count = 3
    assert!(!state.is_borrowed());

    let borrowed = state.borrow();
    assert!(borrowed.is_borrowed());
    assert_eq!(borrowed.count(), 3); // RC unchanged

    let unborrowed = borrowed.unborrow();
    assert!(!unborrowed.is_borrowed());
    assert_eq!(unborrowed.count(), 3);
}

#[test]
fn test_state_overflow() {
    // Create a state at max RC
    let state = State(State::RC_MAX);

    // Try to increment should fail
    let result = state.inc();
    assert!(result.is_none());
}

#[test]
fn test_state_borrow_preserves_rc() {
    let state = State::new().inc().unwrap().inc().unwrap(); // count = 3
    let original_count = state.count();

    let borrowed = state.borrow();
    assert_eq!(borrowed.count(), original_count);

    let unborrowed = borrowed.unborrow();
    assert_eq!(unborrowed.count(), original_count);
}

#[test]
fn test_state_eq() {
    let state1 = State::new();
    let state2 = State::new();
    assert_eq!(state1, state2);

    let state3 = state1.inc().unwrap();
    assert_ne!(state1, state3);

    let state4 = state1.borrow();
    assert_ne!(state1, state4);
}
