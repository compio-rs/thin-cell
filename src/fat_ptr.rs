#[repr(C)]
union FatPtrUnion<T: ?Sized> {
    ptr: *const T,
    component: FatPtr,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FatPtr {
    pub ptr: *mut (),
    pub metadata: usize,
}

pub const fn is_fat<T: ?Sized>() -> bool {
    size_of::<*const T>() != size_of::<*const ()>()
}

impl FatPtr {
    const fn assert_fat<T: ?Sized>() {
        const {
            assert!(
                is_fat::<T>(),
                "`T` must be a `!Thin` type, i.e., `*const T` must be a fat pointer"
            );
        }
    }

    /// Converts a raw fat pointer to its components.
    pub const fn from_ptr<T: ?Sized>(ptr: *const T) -> Self {
        Self::assert_fat::<T>();

        let fat = FatPtrUnion { ptr };
        unsafe { fat.component }
    }

    /// Converts the components back into a raw fat pointer.
    pub const fn into_ptr<T: ?Sized>(self) -> *const T {
        let fat = FatPtrUnion { component: self };
        unsafe { fat.ptr }
    }
}
