#![allow(clippy::missing_safety_doc)]

pub use glib;
pub use qrtr_sys as ffi;

mod auto;

pub use auto::*;
