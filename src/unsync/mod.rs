//! Singlethreaded version of `ThinCell`

mod state;
use state::*;

crate::thin_cell! {
    /// A compact (`1-usize`), single-threaded smart pointer combining `Rc`
    /// and `RefCell` with only `borrow_mut`.
}
