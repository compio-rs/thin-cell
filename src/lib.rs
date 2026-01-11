#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::{
    any::{Any, TypeId},
    cell::{Cell, UnsafeCell},
    fmt::{self, Debug, Display},
    marker::PhantomData,
    mem::{ManuallyDrop, size_of},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

mod state;
use state::State;

mod fat_ptr;
use fat_ptr::FatPtr;

/// The inner allocation of `ThinCell`
///
/// This should not be used except in unsize coercion solely as a type.
#[repr(C)]
pub struct Inner<T: ?Sized> {
    // metadata MUST be at offset 0 so that `*mut Inner<T>` is also a valid `*mut usize` points
    // to the metadata
    metadata: usize,
    state: Cell<State>,
    data: UnsafeCell<T>,
}

/// A compact (`1-usize`), single-threaded smart pointer combining `Rc`
/// and `borrow_mut`-only `RefCell`
pub struct ThinCell<T: ?Sized> {
    ptr: NonNull<()>,
    _marker: PhantomData<Inner<T>>,
}

/// A mutable guard returned by [`ThinCell::borrow`]
pub struct Ref<'a, T: ?Sized> {
    value: &'a mut T,
    state_cell: &'a Cell<State>,
}

impl<T> ThinCell<T> {
    /// Creates a new `ThinCell` wrapping the given data.
    pub fn new(data: T) -> Self {
        let inner = Inner {
            metadata: 0,
            state: Cell::new(State::new()),
            data: UnsafeCell::new(data),
        };

        let ptr = Box::into_raw(Box::new(inner));

        ThinCell {
            ptr: unsafe { NonNull::new_unchecked(ptr as _) },
            _marker: PhantomData,
        }
    }

    /// Consumes the `ThinCell` and try to get inner value.
    ///
    /// Returns the inner value in [`Ok`] if there are no other owners and it is
    /// not currently borrowed, return `Err(self)` otherwise.
    pub fn try_unwrap(self) -> Result<T, Self> {
        let inner = self.inner();
        let s = inner.state.get();

        if s.count() != 1 || s.is_borrowed() {
            return Err(self);
        }

        // SAFETY: As tested above, there are no other owners and it is not borrowed
        Ok(unsafe { self.unwrap_unchecked() })
    }

    /// Consumes the `ThinCell`, returning the inner value.
    ///
    /// # Safety
    /// The caller must guarantee that there are no other owners and it is not
    /// currently borrowed.
    pub unsafe fn unwrap_unchecked(self) -> T {
        let this = ManuallyDrop::new(self);
        // SAFETY: guaranteed by caller to have unique ownership and is not borrowed
        let inner = unsafe { Box::from_raw(this.inner_ptr() as *mut Inner<T>) };

        inner.data.into_inner()
    }
}

impl<T: ?Sized> ThinCell<T> {
    const SIZED: bool = size_of::<*const Inner<T>>() == size_of::<usize>();

    /// Reconstructs the raw pointer to the inner allocation.
    fn inner_ptr(&self) -> *const Inner<T> {
        unsafe {
            let ptr = self.ptr.as_ptr();

            if Self::SIZED {
                // SIZED CASE: Cast pointer-to-pointer
                // Doing this trick to workaround Rust not allowing `ptr as *const Inner<T>`
                // due to `T` being `?Sized` directly even when we know it's `Sized`
                let ptr_ref = &ptr as *const *mut () as *const *const Inner<T>;
                *ptr_ref
            } else {
                // UNSIZED CASE: Read metadata
                let metadata = *(ptr as *const usize);

                // Miri will complain about this:
                // - https://github.com/thepowersgang/stack_dst-rs/issues/14
                // - https://github.com/uazu/stakker/blob/5821c30409c19ca9167808b669c928c94bc5f177/src/queue/flat.rs#L14-L17
                // But this should be sound as per Rust's fat pointer and metadata construction
                FatPtr { ptr, metadata }.into_ptr()
            }
        }
    }

    /// Returns a reference to the inner allocation.
    fn inner(&self) -> &Inner<T> {
        unsafe { &*self.inner_ptr() }
    }

    /// Deallocates the inner allocation.
    ///
    /// # Safety
    /// `self` must be the last owner and it must not be used after this call.
    unsafe fn drop_in_place(&mut self) {
        drop(unsafe { Box::from_raw(self.inner_ptr() as *mut Inner<T>) })
    }

    /// Leaks the `ThinCell`, returning a raw pointer to the inner allocation.
    ///
    /// The returned pointer points to the inner allocation. To restore the
    /// `ThinCell`, use [`ThinCell::from_raw`].
    pub fn leak(self) -> *mut () {
        let this = ManuallyDrop::new(self);
        this.ptr.as_ptr()
    }

    /// Reconstructs a `ThinCell<T>` from a raw pointer.
    ///
    /// # Safety
    /// The pointer must have been obtained from a previous call to
    /// [`ThinCell::leak`], and the [`ThinCell`] must not have been dropped in
    /// the meantime.
    pub unsafe fn from_raw(ptr: *mut ()) -> Self {
        ThinCell {
            // SAFETY: caller guarantees `ptr` is valid
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            _marker: PhantomData,
        }
    }

    /// Downcasts the `ThinCell<T>` to `ThinCell<U>`.
    ///
    /// # Safety
    /// The caller must ensure that the inner value is actually of type `U`.
    pub unsafe fn downcast_unchecked<U>(self) -> ThinCell<U> {
        let this = ManuallyDrop::new(self);
        ThinCell {
            ptr: this.ptr,
            _marker: PhantomData,
        }
    }

    /// Returns the number of owners.
    pub fn count(&self) -> usize {
        self.inner().state.get().count()
    }

    /// Borrows the value mutably.
    ///
    /// Returns a [`Ref`] guard that provides mutable access to the inner value.
    /// The borrow lasts until the guard is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the value is already borrowed. For a non-panicking variant,
    /// use [`try_borrow`](ThinCell::try_borrow).
    ///
    /// # Examples
    ///
    /// ```
    /// # use thin_cell::ThinCell;
    /// let cell = ThinCell::new(5);
    ///
    /// {
    ///     let mut borrowed = cell.borrow();
    ///     *borrowed = 10;
    /// } // borrow is released here
    ///
    /// assert_eq!(*cell.borrow(), 10);
    /// ```
    pub fn borrow(&self) -> Ref<'_, T> {
        let inner = self.inner();
        inner.state.update(State::borrow);

        // SAFETY: We have exclusive access via borrow flag
        let value = unsafe { &mut *inner.data.get() };

        Ref {
            value,
            state_cell: &inner.state,
        }
    }

    /// Attempts to borrow the value mutably.
    ///
    /// Returns `Some(Ref)` if the value is not currently borrowed, or `None` if
    /// it is already borrowed.
    ///
    /// This is the non-panicking variant of [`borrow`](ThinCell::borrow).
    ///
    /// # Examples
    ///
    /// ```
    /// # use thin_cell::ThinCell;
    /// let cell = ThinCell::new(5);
    ///
    /// let borrow1 = cell.borrow();
    /// assert!(cell.try_borrow().is_none()); // Already borrowed
    /// drop(borrow1);
    /// assert!(cell.try_borrow().is_some()); // Now available
    /// ```
    pub fn try_borrow(&self) -> Option<Ref<'_, T>> {
        let inner = self.inner();
        let state = inner.state.get().try_borrow()?;
        inner.state.set(state);

        // SAFETY: We have exclusive access via borrow flag
        let value = unsafe { &mut *inner.data.get() };

        Some(Ref {
            value,
            state_cell: &inner.state,
        })
    }

    /// Get a mutable reference to the inner value without any checks.
    ///
    /// # Safety
    /// The caller must guarantee that there are no other owners and it is not
    /// currently borrowed.
    pub unsafe fn borrow_unchecked(&mut self) -> &mut T {
        let inner = self.inner();
        unsafe { &mut *inner.data.get() }
    }

    /// Creates a new `ThinCell<U>` from `data: U` and coerces it to
    /// `ThinCell<T>`.
    ///
    /// # Safety
    /// `coerce` function must ensure the returned pointer is:
    /// - a valid unsizing of `T`, e.g., some `dyn Trait` with concrete type `U`
    /// - with same address (bare data pointer without metadata) as input
    pub unsafe fn new_unsize<U>(
        data: U,
        coerce: impl Fn(*const Inner<U>) -> *const Inner<T>,
    ) -> Self {
        let this = ThinCell::new(data);
        // SAFETY: We're holding unique ownership and is not borrowed.
        unsafe { this.unsize_unchecked(coerce) }
    }

    /// Manually coerce to unsize with some checks.
    ///
    /// # Safety
    /// `coerce` function must ensure the returned pointer is:
    /// - a valid unsizing of `T`, e.g., some `dyn Trait` with concrete type `U`
    /// - with same address (bare data pointer without metadata) as input
    ///
    /// See [`ThinCell::unsize_unchecked`] for details.
    pub unsafe fn unsize<U: ?Sized>(
        self,
        coerce: impl Fn(*const Inner<T>) -> *const Inner<U>,
    ) -> ThinCell<U> {
        let inner = self.inner();
        let s = inner.state.get();

        assert!(!s.is_shared(), "Cannot coerce shared `ThinCell`");
        assert!(!s.is_borrowed(), "Cannot coerce borrowed `ThinCell`");

        // SAFETY: As tested above, the `ThinCell` is:
        // - not shared, and
        // - not borrowed
        // - validity of `coerce` is guaranteed by caller
        unsafe { self.unsize_unchecked(coerce) }
    }

    /// Manually coerce to unsize without checks
    ///
    /// The returned `U` must be a valid unsizing of `T`, i.e., some `dyn
    /// Trait` with concrete type `T`. `&U` must be a fat pointer. If not, this
    /// will fail to compile.
    ///
    /// # Safety
    ///
    /// The `ThinCell` must:
    /// - have unique ownership (count == 1)
    /// - not be borrowed
    ///
    /// In particular, this is the exact state after [`ThinCell::new`].
    ///
    /// `coerce` function must ensure the returned pointer:
    /// - is a valid unsizing of `T`, e.g., some `dyn Trait` with concrete type
    ///   `U`
    /// - consists the same address (bare data pointer without metadata) as the
    ///   input
    pub unsafe fn unsize_unchecked<U: ?Sized>(
        self,
        coerce: impl Fn(*const Inner<T>) -> *const Inner<U>,
    ) -> ThinCell<U> {
        let this = ManuallyDrop::new(self);

        let old_ptr = this.inner_ptr();
        let fat_ptr = coerce(old_ptr);

        let FatPtr { ptr, metadata } = FatPtr::from_ptr::<Inner<U>>(fat_ptr);

        // SAFETY: `Inner` is `repr(C)` and has `metadata` at offset 0
        unsafe { *(old_ptr as *mut usize) = metadata };

        ThinCell {
            // SAFETY: `ptr` is valid as it comes from `self`
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            _marker: PhantomData,
        }
    }

    /// Returns the raw pointer to the inner allocation.
    pub fn as_ptr(&self) -> *const () {
        self.ptr.as_ptr()
    }

    /// Returns `true` if the two `ThinCell`s point to the same allocation.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.as_ptr(), other.as_ptr())
    }
}

impl<T: Any + ?Sized> ThinCell<T> {
    /// Attempts to downcast the `ThinCell<T>` to `ThinCell<U>`.
    ///
    /// Returns `Some(ThinCell<U>)` if the inner value is of type `U`, or
    /// `None` otherwise.
    pub fn downcast<U: Any>(self) -> Option<ThinCell<U>> {
        let inner = self.inner();
        let data_ref = unsafe { &*inner.data.get() };

        if TypeId::of::<U>() == data_ref.type_id() {
            // SAFETY: We have verified that the inner value is of type `U`
            Some(unsafe { self.downcast_unchecked::<U>() })
        } else {
            None
        }
    }
}

/// `ThinCell` is `Unpin` as it does not move its inner data.
impl<T: ?Sized> Unpin for ThinCell<T> {}

impl<'a, T: ?Sized> Drop for Ref<'a, T> {
    fn drop(&mut self) {
        let current = self.state_cell.get();
        self.state_cell.set(current.unborrow());
    }
}

impl<'a, T: ?Sized> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, T: ?Sized> DerefMut for Ref<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<'a, T: Debug + ?Sized> Debug for Ref<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&**self, f)
    }
}

impl<'a, T: Display + ?Sized> Display for Ref<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Clone for ThinCell<T> {
    fn clone(&self) -> Self {
        unsafe {
            let inner = &*self.inner_ptr();
            let current = inner.state.get();

            match current.inc() {
                Some(new_state) => inner.state.set(new_state),
                None => panic!("Reference count overflow"),
            }

            ThinCell {
                ptr: self.ptr,
                _marker: PhantomData,
            }
        }
    }
}

impl<T: ?Sized> Drop for ThinCell<T> {
    fn drop(&mut self) {
        unsafe {
            let ptr = self.inner_ptr();
            // SAFETY: pointer returned by `inner_ptr` is valid
            let inner = &*ptr;
            let current = inner.state.get();

            // If count is 1, we are the last owner
            if current.count() == 1 {
                debug_assert!(!current.is_borrowed(), "Dropping while borrowed");
                self.drop_in_place();
            } else {
                // Not last owner, decrement
                inner.state.set(current.dec());
            }
        }
    }
}

impl<T: Default> Default for ThinCell<T> {
    fn default() -> Self {
        ThinCell::new(T::default())
    }
}

impl<T: Debug + ?Sized> Debug for ThinCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner();
        let state = inner.state.get();
        let mut d = f.debug_struct("ThinCell");
        match self.try_borrow() {
            Some(borrowed) => d.field("value", &borrowed),
            None => d.field("value", &"<borrowed>"),
        }
        .field("state", &state)
        .finish()
    }
}

impl<T: Display + ?Sized> Display for ThinCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_borrow() {
            Some(borrowed) => Display::fmt(&*borrowed, f),
            None => write!(f, "<borrowed>"),
        }
    }
}

impl<T: PartialEq + ?Sized> PartialEq<ThinCell<T>> for ThinCell<T> {
    /// Compares the inner values for equality.
    ///
    /// # Panics
    ///
    /// Panics if either `ThinCell` is currently borrowed.
    fn eq(&self, other: &Self) -> bool {
        self.borrow().eq(&other.borrow())
    }
}

impl<T: Eq + ?Sized> Eq for ThinCell<T> {}

impl<T: Ord + ?Sized> PartialOrd<ThinCell<T>> for ThinCell<T> {
    /// Compares the inner values.
    ///
    /// # Panics
    ///
    /// Panics if either `ThinCell` is currently borrowed.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.borrow().partial_cmp(&other.borrow())
    }
}
