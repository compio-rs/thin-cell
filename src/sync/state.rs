use synchrony::sync::atomic::AtomicUsize;

use crate::state::*;

impl_state! {
    /// Internally synchronized and uses spin-lock for borrow operations.
    struct State(AtomicUsize);
}

impl State {
    pub fn acquire(&self) {
        self.0.load(Acquire);
    }

    pub fn borrow(&self) {
        let mut curr = self.load().0;

        loop {
            let old = curr & !BORROW_MASK;
            let new = curr | BORROW_MASK;

            match self.0.compare_exchange_weak(old, new, Acquire, Relaxed) {
                Ok(_) => return,
                Err(actual) => {
                    std::thread::yield_now();
                    curr = actual;
                    continue;
                }
            }
        }
    }

    /// Tries to set the borrow bit. Returns `true` if successful, `false` if
    /// already borrowed.
    #[inline]
    pub fn try_borrow(&self) -> bool {
        self.0
            .fetch_update(Acquire, Relaxed, |curr| {
                if (curr & BORROW_MASK) != 0 {
                    None // Already borrowed, fail
                } else {
                    Some(curr | BORROW_MASK) // Set borrow bit
                }
            })
            .is_ok()
    }
}

test_cases!(AtomicUsize);
