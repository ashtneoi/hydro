use std::mem::{align_of, size_of};
use std::ptr;

/// *extremely* unsafe
pub(crate) unsafe fn steal_mut<'a, T>(x: &mut T) -> &'a mut T {
    &mut *(x as *mut T)
}

pub(crate) fn align_mut_ptr_down<T>(p: *mut T, alignment: usize) -> *mut T {
    ((p as usize) & !((1<<alignment) - 1)) as *mut T
}

// TODO: Use preferred alignment?
/// p need not be aligned.
pub(crate) unsafe fn push_raw<T>(p: &mut *mut u8, val: T) {
    *p = align_mut_ptr_down(*p, align_of::<T>());
    *p = ((*p as usize) - size_of::<T>()) as *mut u8;
    ptr::write(*p as *mut T, val);
}
