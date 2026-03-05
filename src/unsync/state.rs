use synchrony::unsync::atomic::AtomicUsize;

use crate::state::*;

impl_state! {
    /// Unsynchronized and intended for single-threaded use.
    struct State(AtomicUsize);
}

impl State {
    pub fn acquire(&self) {}

    pub fn borrow(&self) {
        let curr = self.load().0;
        if (curr & BORROW_MASK) != 0 {
            panic!("Already borrowed");
        } else {
            self.0.store(curr | BORROW_MASK, Release);
        }
    }

    /// Tries to set the borrow bit. Returns `true` if successful, `false` if
    /// already borrowed.
    #[inline]
    pub fn try_borrow(&self) -> bool {
        if self.load().is_borrowed() {
            return false;
        }

        self.0.fetch_or(BORROW_MASK, Release);
        true
    }
}

test_cases!(AtomicUsize);

#[test]
#[should_panic(expected = "Already borrowed")]
fn test_state_borrow_panic() {
    let state = State::new();
    state.borrow();
    state.borrow(); // Should panic
}
