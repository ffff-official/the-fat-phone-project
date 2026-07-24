//! **libqmi-glib-rs provides safe, asynchronous Rust bindings for libqmi-glib.**
//!
//! It allows applications to communicate with QMI-based cellular modems using idiomatic async/await while leveraging GLib's asynchronous APIs under the hood.
//! The library wraps a little part of QMI operations such as device management, client creation, and network services in a Rust-friendly API.  
//! ```rust
//! use libqmi_glib_rs::*;
//!
//! use gio_sys::g_file_new_for_path;
//! use glib::MainLoop;
//! use gobject_sys::g_object_unref;
//!
//! use std::ffi::CString;
//!
//! fn main() {
//!     let ctx = glib::MainContext::default();
//!     let main_loop = MainLoop::new(Some(&ctx), false);
//!     let main_loop_clone = main_loop.clone();
//!
//!     let path = CString::new("/dev/cdc-wdm1").unwrap();
//!     let file = unsafe { g_file_new_for_path(path.as_ptr()) };
//!
//!     ctx.spawn_local(async move {
//!         let mut device = Device::new(file).await?;
//!         device.open().await?;
//!
//!         let wds_client = WdsClient::new(&device).await?;
//!         let status = wds_client.get_packet_service_status().await?;
//!
//!         println!("Packet service status: {:?}", status);
//!
//!         wds_client.close().await?;
//!         device.close().await?;
//!
//!         main_loop_clone.quit();
//!
//!         Ok::<(), Box<dyn std::error::Error>>(())
//!     });
//!
//!     main_loop.run();
//!
//!     unsafe {
//!         g_object_unref(file as _);
//!     }
//! }
//! ```
#![allow(unsafe_op_in_unsafe_fn)] // Warnings on unsafe_op_in_unsafe_fn is a shitty idea for a crate of this kind.

macro_rules! allocate_client_callback {
    (
        $callback:ident,
        $client_type:ty,
        $context:literal
    ) => {
        unsafe extern "C" fn $callback(
            device: *mut GObject,
            res: *mut GAsyncResult,
            user_data: *mut c_void,
        ) {
            let sender = Box::from_raw(
                user_data.cast::<oneshot::Sender<QmiResult<NonNull<$client_type>>>>(),
            );

            let mut error: *mut GError = null_mut();

            let client = qmi_device_allocate_client_finish(device.cast(), res, &mut error);

            let output =
                NonNull::new(client.cast::<$client_type>()).ok_or_else(|| (error, $context).into());

            drop(sender.send(output).inspect_err(cannot_send));
        }
    };
}

macro_rules! release_client_callback {
    (
        $callback:ident,
        $context:literal
    ) => {
        unsafe extern "C" fn $callback(
            device: *mut GObject,
            res: *mut GAsyncResult,
            user_data: *mut c_void,
        ) {
            let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
            let mut error: *mut GError = null_mut();

            let res = qmi_device_release_client_finish(device.cast(), res, &mut error);

            let output = match res {
                GFALSE => Err((error, $context).into()),
                _ => Ok(()),
            };
            drop(sender.send(output).inspect_err(cannot_send));
        }
    };
}

pub mod dms_client;
pub mod nas_client;
pub mod pds_client;
pub mod qmi_device;
pub mod uim_client;
pub mod voice_client;
pub mod wds_client;
pub mod wms_client;

/// Re-exports all definitions from libqmi-glib-sys.
pub mod sys {
    pub use libqmi_glib_sys::*;
}
/// Re-exports all definitions from libqmi-glib-types-rs.
pub mod types {
    pub use libqmi_glib_types_rs::*;
}

#[doc(inline)]
pub use dms_client::DmsClient;
#[doc(inline)]
pub use nas_client::NasClient;
#[doc(inline)]
pub use pds_client::PdsClient;
#[doc(inline)]
pub use qmi_device::Device;
#[doc(inline)]
pub use uim_client::UimClient;
#[doc(inline)]
pub use voice_client::VoiceClient;
#[doc(inline)]
pub use wds_client::WdsClient;
#[doc(inline)]
pub use wms_client::WmsClient;

use glib_sys::{GError, g_clear_error};

const OPERATION_TIMEOUT: u32 = 5;

fn cannot_send<T: std::fmt::Debug>(result: &QmiResult<T>) {
    tracing::error!("Cannot send : {:?}", result);
}

pub type QmiResult<T> = Result<T, QmiError>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct QmiError {
    pub message: String,
    pub context: &'static str,
}

impl std::fmt::Display for QmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}

impl QmiError {
    fn new<T: ToString>(message: T, context: &'static str) -> Self {
        Self {
            message: message.to_string(),
            context,
        }
    }
}

impl std::error::Error for QmiError {}

#[doc(hidden)]
impl From<(*mut GError, &'static str)> for QmiError {
    fn from((mut error, context): (*mut GError, &'static str)) -> Self {
        unsafe {
            let ret = Self {
                message: std::ffi::CStr::from_ptr((*error).message)
                    .to_string_lossy()
                    .to_string(),
                context,
            };
            g_clear_error(&mut error);
            ret
        }
    }
}

#[doc(hidden)]
impl<T: std::error::Error> From<(T, &'static str, ())> for QmiError {
    fn from((error, context, _): (T, &'static str, ())) -> Self {
        Self {
            message: error.to_string(),
            context,
        }
    }
}

impl QmiError {
    fn cb_droped() -> Self {
        Self {
            message: "cb droped".to_string(),
            context: "Unknown",
        }
    }
}

fn tracing_debug_hex(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
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
