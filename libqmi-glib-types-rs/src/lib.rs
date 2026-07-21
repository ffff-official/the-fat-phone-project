#![feature(doc_cfg)]
#![allow(clippy::missing_safety_doc)]

pub use gio;
pub use glib;
pub use libqmi_glib_sys as ffi;

mod auto;
mod structs;

pub use auto::*;
pub use structs::*;
