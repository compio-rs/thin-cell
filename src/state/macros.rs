macro_rules! impl_state {
    {
        $( #[$meta:meta] )*
        struct State($inner:ident);
    } => {
        use std::{process::abort, sync::atomic::Ordering::*};

        /// Encapsulates the bitwise logic for Reference Counting and borrow flags.
        ///
        /// All bits except last are used for Reference Count (RC), while last bit is
        /// used for borrow flags (Borrowed).
        ///
        $( #[$meta] )*
        #[repr(transparent)]
        pub struct State($inner);

        impl std::fmt::Debug for State {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple("State").field(&self.load()).finish()
            }
        }

        impl State {
            pub fn new() -> Self {
                // Starts with 1 owner, 0 borrows
                State($inner::new(RC_UNIT))
            }

            pub fn load(&self) -> Snapshot {
                self.0.load(Relaxed).into()
            }

            pub fn inc(&self) -> &Self {
                // As explained in `Arc`'s comment, use relaxed ordering is fine for
                // reference count increment.
                let orig = self.0.fetch_add(RC_UNIT, Relaxed);

                // Quote unquote from `Arc`:
                // > This branch will never be taken in any realistic program. We abort because
                // > such a program is incredibly degenerate, and we don't care to support it.
                if (orig & RC_MASK) == RC_MASK {
                    abort()
                }

                self
            }

            /// Decrease reference count by one.
            ///
            /// Returns whether reference count has reached zero (needs drop).
            pub fn dec(&self) -> bool {
                // Because `fetch_sub` is already atomic, we do not need to synchronize
                // with other threads unless we are going to delete the object.
                if self.0.fetch_sub(RC_UNIT, Release) != RC_UNIT {
                    return false;
                }

                debug_assert!(
                    !self.load().is_borrowed(),
                    "Reference count should never reach zero while borrowed"
                );

                // Prevent any other thread from reading after we have decremented the
                // count to zero, which could lead to use-after-free.
                self.acquire();

                true
            }
            /// Try to zero the reference count if there is only one owner and not borrowed.
            ///
            /// Returns whether the unwrapping is successful (i.e., we can safely take the underlying object).
            pub fn try_unwrap(&self) -> bool {
                // Only when `state == RC_UNIT` (one owner, not borrowed) can we safely unwrap.
                // Any other state means either multiple owners or borrowed, both of which
                // prevent unwrapping.
                if self
                    .0
                    .compare_exchange(RC_UNIT, 0, Release, Relaxed)
                    .is_err()
                {
                    return false;
                }

                // Similar to `dec`, we need to synchronize with other threads to prevent them from reading the
                // object after we have took it.
                self.acquire();

                true
            }

            pub fn unborrow(&self) {
                // Keep RC bits, clear Borrow bits
                self.0.fetch_and(RC_MASK, Release);
            }
        }
    };
}

macro_rules! test_cases {
    ($usize:ty) => {
        #[test]
        fn test_state_new() {
            let state = State::new().load();
            assert_eq!(state.count(), 1);
            assert!(!state.is_borrowed());
            assert!(!state.is_shared());
        }

        #[test]
        fn test_state_count() {
            let state = State::new();
            assert_eq!(state.load().count(), 1);

            state.inc();
            assert_eq!(state.load().count(), 2);

            state.inc();
            assert_eq!(state.load().count(), 3);
        }

        #[test]
        fn test_state_inc() {
            let state = State::new();
            assert_eq!(state.load().count(), 1);

            for i in 2..=10 {
                state.inc();
                assert_eq!(state.load().count(), i);
            }
        }

        #[test]
        fn test_state_dec() {
            let state = State::new();
            state.inc();
            state.inc();
            state.inc(); // count = 4

            state.dec(); // count = 3
            assert_eq!(state.load().count(), 3);

            state.dec(); // count = 2
            assert_eq!(state.load().count(), 2);

            state.dec(); // count = 1
            assert_eq!(state.load().count(), 1);
        }

        #[test]
        fn test_state_is_shared() {
            let state = State::new();
            assert!(!state.load().is_shared());

            state.inc();
            assert!(state.load().is_shared());

            state.inc();
            assert!(state.load().is_shared());

            state.dec();
            assert!(state.load().is_shared());

            state.dec();
            assert!(!state.load().is_shared());
        }

        #[test]
        fn test_state_borrow() {
            let state = State::new();
            assert!(!state.load().is_borrowed());

            state.borrow();
            let borrowed = state.load();
            assert!(borrowed.is_borrowed());
            assert_eq!(borrowed.count(), 1); // RC unchanged
        }

        #[test]
        fn test_state_try_borrow_success() {
            let state = State::new();
            let success = state.try_borrow();
            assert!(success);

            let borrowed = state.load();
            assert!(borrowed.is_borrowed());
            assert_eq!(borrowed.count(), 1);
        }

        #[test]
        fn test_state_try_borrow_failure() {
            let state = State::new();
            state.borrow();

            // Already borrowed, should fail
            let success = state.try_borrow();
            assert!(!success);
        }

        #[test]
        fn test_state_unborrow() {
            let state = State::new();
            state.borrow();
            assert!(state.load().is_borrowed());

            state.unborrow();
            let unborrowed = state.load();
            assert!(!unborrowed.is_borrowed());
            assert_eq!(unborrowed.count(), state.load().count()); // RC unchanged
        }

        #[test]
        fn test_state_borrow_with_multiple_refs() {
            let state = State::new();
            state.inc();
            state.inc(); // count = 3
            assert!(!state.load().is_borrowed());

            state.borrow();
            let borrowed = state.load();
            assert!(borrowed.is_borrowed());
            assert_eq!(borrowed.count(), 3); // RC unchanged

            state.unborrow();
            let unborrowed = state.load();
            assert!(!unborrowed.is_borrowed());
            assert_eq!(unborrowed.count(), 3);
        }

        #[test]
        fn test_state_borrow_preserves_rc() {
            let state = State::new();
            state.inc();
            state.inc(); // count = 3
            let original_count = state.load().count();

            state.borrow();
            assert_eq!(state.load().count(), original_count);

            state.unborrow();
            assert_eq!(state.load().count(), original_count);
        }

        #[test]
        fn test_state_eq() {
            let state1 = State::new();
            let state2 = State::new();
            assert_eq!(state1.load(), state2.load());

            state1.inc();
            assert_ne!(state1.load(), state2.load());

            state1.dec();
            assert_eq!(state1.load(), state2.load());

            state1.borrow();
            assert_ne!(state1.load(), state2.load());
        }
    };
}

pub(crate) use impl_state;
pub(crate) use test_cases;
