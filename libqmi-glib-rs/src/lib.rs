//! **libqmi-glib-rs provides safe, asynchronous Rust bindings for libqmi-glib.**
//!
//! It allows applications to communicate with QMI-based cellular modems using idiomatic async/await while leveraging GLib's asynchronous APIs under the hood.
//! The library wraps a little part of QMI operations such as device management, client creation, and network services in a Rust-friendly API.  
//! ```no_run
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
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(unsafe_op_in_unsafe_fn)] // Warnings on unsafe_op_in_unsafe_fn is a shitty idea for a crate of this kind.

#[doc(hidden)]
pub(crate) mod __macro_support {
    pub use crate::sys::*;
    pub use futures::channel::oneshot;
    pub use gio_sys::GAsyncResult;
    pub use glib_sys::GFALSE;
    pub use gobject_sys::{GObject, g_object_ref, g_object_unref};
    pub use libqmi_glib_types_rs::DeviceReleaseClientFlags;
    pub use libqmi_glib_types_rs::glib::translate::IntoGlib;

    pub use std::ffi::c_void;
    pub use std::ptr::{NonNull, null_mut};
}

// macro_rules! function {
//     () => {{
//         fn f() {}
//         fn type_name_of<T>(_: T) -> &'static str {
//             std::any::type_name::<T>()
//         }
//         let name = type_name_of(f);
//         name.strip_suffix("::f").unwrap()
//     }}
// }

macro_rules! client_drop_impl {
    ($client_type:ty) => {
        impl Drop for $client_type {
            fn drop(&mut self) {
                tracing::trace!("{} is droped !", stringify!($client_type));
                unsafe {
                    $crate::__macro_support::g_object_unref(self.client.as_ptr().cast());
                }
            }
        }
    };
}

macro_rules! client_clone_impl {
    ($client_type:ty) => {
        impl Clone for $client_type {
            fn clone(&self) -> Self {
                unsafe {
                    $crate::__macro_support::g_object_ref(self.client.as_ptr().cast());
                }

                Self {
                    client: self.client,
                    device: self.device.clone(),
                    data: self.data.clone(),
                }
            }
        }
    };
}

macro_rules! new_client_impl {
    (
        $client_type:ty,
        $ffi_client_type:ty,
        $service:expr
    ) => {
        /// Asynchronously allocates a new [`Self`].
        pub async fn new(device: &Device) -> QmiResult<$client_type> {
            let (tx, rx) =
                $crate::__macro_support::oneshot::channel::<QmiResult<NonNull<$ffi_client_type>>>();

            unsafe {
                $crate::__macro_support::qmi_device_allocate_client(
                    device.as_ptr(),
                    $crate::__macro_support::IntoGlib::into_glib($service),
                    $crate::__macro_support::QMI_CID_NONE as _,
                    $crate::OPERATION_TIMEOUT,
                    $crate::__macro_support::null_mut(),
                    Some(allocate_client_cb),
                    Box::into_raw(Box::new(tx)).cast(),
                )
            };
            match rx.await.map_err(|_| QmiError::cb_droped())? {
                Ok(client) => Ok(Self {
                    device: device.clone(),
                    client,
                    data: Rc::new(RefCell::new(None)),
                }),
                Err(e) => Err(e),
            }
        }
    };
}

macro_rules! close_client_impl {
    ($context:literal) => {
        /// Asynchronously releases the client from the [`Device`].
        pub async fn close(self) -> QmiResult<()> {
            let ref_count = unsafe {
                *(self
                    .client
                    .as_ptr()
                    .cast::<$crate::__macro_support::GObject>())
            }
            .ref_count;
            if ref_count != 2 {
                Err(QmiError::new(
                    format!("can't close ref count = {}", ref_count),
                    concat!($context, " close"),
                ))?;
            }
            let (tx, rx) = $crate::__macro_support::oneshot::channel::<QmiResult<()>>();

            use $crate::__macro_support::IntoGlib;
            unsafe {
                $crate::__macro_support::qmi_device_release_client(
                    self.device.as_ptr(),
                    self.client.as_ptr().cast(),
                    $crate::__macro_support::DeviceReleaseClientFlags::RELEASE_CID.into_glib(),
                    $crate::OPERATION_TIMEOUT,
                    $crate::__macro_support::null_mut(),
                    Some(release_client_cb),
                    Box::into_raw(Box::new(tx)).cast(),
                );
            }
            rx.await.map_err(|_| QmiError::cb_droped())?
        }
    };
}

macro_rules! allocate_client_callback {
    (
        $callback:ident,
        $client_type:ty,
        $context:literal
    ) => {
        unsafe extern "C" fn $callback(
            device: *mut $crate::__macro_support::GObject,
            res: *mut $crate::__macro_support::GAsyncResult,
            user_data: *mut $crate::__macro_support::c_void,
        ) {
            let sender = Box::from_raw(
                user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<NonNull<$client_type>>>>(),
            );

            let mut error: *mut GError = $crate::__macro_support::null_mut();

            let client = $crate::__macro_support::qmi_device_allocate_client_finish(device.cast(), res, &mut error);

            let output =
                $crate::__macro_support::NonNull::new(client.cast::<$client_type>()).ok_or_else(|| (error, $context).into());

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
            device: *mut $crate::__macro_support::GObject,
            res: *mut $crate::__macro_support::GAsyncResult,
            user_data: *mut $crate::__macro_support::c_void,
        ) {
            let sender = Box::from_raw(
                user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<()>>>(),
            );
            let mut error: *mut GError = $crate::__macro_support::null_mut();

            let res = $crate::__macro_support::qmi_device_release_client_finish(
                device.cast(),
                res,
                &mut error,
            );

            let output = match res {
                $crate::__macro_support::GFALSE => Err((error, $context).into()),
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
    fn new(message: impl AsRef<str>, context: &'static str) -> Self {
        Self {
            message: message.as_ref().to_string(),
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
