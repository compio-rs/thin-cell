//! Multithreaded version of `ThinCell`

mod state;
use state::*;

crate::thin_cell! {
    /// A compact (`1-usize`), multi-threaded smart pointer combining `Arc`
    /// and `Mutex`.
}

unsafe impl<T: ?Sized + Send + Sync> Send for ThinCell<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for ThinCell<T> {}
