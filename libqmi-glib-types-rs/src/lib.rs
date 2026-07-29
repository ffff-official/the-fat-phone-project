#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::missing_safety_doc)]

pub use gio;
pub use glib;
use glib_sys::GArray;
pub use libqmi_glib_sys as ffi;

mod auto;
mod structs;

pub use auto::*;
pub use structs::*;

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn slice_to_garray<T>(slice: &[T]) -> *mut GArray
where
    T: Copy,
{
    let len = slice.len();

    let array = glib_sys::g_array_sized_new(0, 0, std::mem::size_of::<T>() as u32, len as u32);

    glib_sys::g_array_set_size(array, len as u32);

    if len != 0 {
        let destination = (*array).data as *mut T;

        for (index, value) in slice.iter().enumerate() {
            destination.add(index).write(*value);
        }
    }
    array
}
