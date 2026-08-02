use glib_sys::GArray;
use libqmi_glib_types_rs::glib::translate::FromGlib;

use crate::QmiResult;

pub(crate) fn cannot_send<T: std::fmt::Debug>(result: &QmiResult<T>) {
    tracing::error!("Cannot send : {:?}", result);
}

pub(crate) fn tracing_debug_hex(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write;

    let mut hex = String::new();
    writeln!(&mut hex, "source raw values :")?;
    for b in source {
        write!(&mut hex, "{:02X} ", b)?;
    }
    hex.pop();

    tracing::debug!("{hex}");
    Ok(())
}

pub(crate) unsafe fn c_char_to_string(value: *const std::ffi::c_char) -> String {
    std::ffi::CStr::from_ptr(value)
        .to_string_lossy()
        .into_owned()
}

pub(crate) unsafe fn garray_primitive_to_vec<T, U>(garray: *const GArray) -> Vec<U>
where
    T: Copy,
    U: From<T>,
{
    if garray.is_null() || (*garray).len == 0 {
        return Vec::new();
    }

    let slice = std::slice::from_raw_parts((*garray).data.cast::<T>(), (*garray).len as usize);

    slice.iter().copied().map(U::from).collect()
}

pub(crate) unsafe fn garray_glib_to_vec<T, U>(garray: *const GArray) -> Vec<U>
where
    T: Copy,
    U: FromGlib<T>,
{
    if garray.is_null() || (*garray).len == 0 {
        return Vec::new();
    }

    let slice = std::slice::from_raw_parts((*garray).data.cast::<T>(), (*garray).len as usize);

    slice
        .iter()
        .copied()
        .map(|elem| U::from_glib(elem))
        .collect()
}
