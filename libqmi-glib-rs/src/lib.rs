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
    pub use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

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
            let (tx, rx) = $crate::__macro_support::oneshot::channel::<
                QmiResult<$crate::__macro_support::NonNull<$ffi_client_type>>,
            >();

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
            let sender = Box::from_raw(user_data.cast::<$crate::__macro_support::oneshot::Sender<
                QmiResult<$crate::__macro_support::NonNull<$client_type>>,
            >>());

            let mut error: *mut GError = $crate::__macro_support::null_mut();

            let client = $crate::__macro_support::qmi_device_allocate_client_finish(
                device.cast(),
                res,
                &mut error,
            );

            let output = $crate::__macro_support::NonNull::new(client.cast::<$client_type>())
                .ok_or_else(|| (error, $context).into());

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

/// Generates an asynchronous getter for a single output value.
///
/// The generated method performs the QMI request, validates its response,
/// and uses the supplied getter function to retrieve the requested value
/// directly from the QMI output message.
///
/// Use `getter_with_extractor!` when retrieving or converting the returned
/// value requires custom logic.
macro_rules! getter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
        $return_type:ty,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl Context {
            $(#[$meta])*
            pub async fn $method_name(&self) -> QmiResult<$return_type> {
                unsafe extern "C" fn callback(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                    user_data: *mut $crate::__macro_support::c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = $crate::__macro_support::null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == $crate::__macro_support::GFALSE {
                        $message_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut flags = 0;
                    let ret = $qmi_function(
                        output,
                        &mut flags,
                        &mut error,
                    );
                    $message_output_unref(output);
                    if ret == $crate::__macro_support::GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok($crate::__macro_support::from_glib(flags))
                }
                let (tx, rx) = $crate::__macro_support::oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        $crate::__macro_support::null_mut(),
                        OPERATION_TIMEOUT,
                        $crate::__macro_support::null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    };
}

/// Generates an asynchronous getter using a custom output extractor.
///
/// The generated method performs the QMI request and validates its response.
/// On success, the supplied extractor is invoked to retrieve and convert the
/// requested value from the QMI output message.
///
/// Use this macro when the returned value requires custom extraction logic,
/// such as converting multiple fields, arrays, or complex QMI types.
macro_rules! getter_with_extractor {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $extractor:path,
        $return_type:ty,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl Context {
            $(#[$meta])*
            pub async fn $method_name(&self) -> QmiResult<$return_type> {
                unsafe extern "C" fn callback(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                    user_data: *mut $crate::__macro_support::c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = $crate::__macro_support::null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == $crate::__macro_support::GFALSE {
                        $message_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut error: *mut GError = $crate::__macro_support::null_mut();
                    let mut final_result = <$return_type>::default();
                    let ret = $extractor(output, &mut error, &mut final_result);

                    $message_output_unref(output);
                    if ret == $crate::__macro_support::GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(final_result)
                }
                let (tx, rx) = $crate::__macro_support::oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        $crate::__macro_support::null_mut(),
                        OPERATION_TIMEOUT,
                        $crate::__macro_support::null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    };
}

/// Generates an asynchronous setter for a single input value.
///
/// The supplied setter function is used to write the method parameter directly
/// into the QMI input message. The generated method performs the request,
/// validates the QMI response, and returns `QmiResult<()>` on completion.
///
/// Use `setter_with_writer!` when populating the input message requires custom
/// logic or multiple values.
macro_rules! setter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
        $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl Context {
            $(#[$meta])*
            pub async fn $method_name(&self, parameter: $parameter_type) -> QmiResult<()> {
                unsafe extern "C" fn callback(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                    user_data: *mut $crate::__macro_support::c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = $crate::__macro_support::null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        $crate::__macro_support::GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    $message_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { $message_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = $crate::__macro_support::null_mut();

                use $crate::__macro_support::IntoGlib;
                let ret = unsafe { $qmi_function(input, parameter.into_glib(), &mut error) };
                if ret == $crate::__macro_support::GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = $crate::__macro_support::oneshot::channel::<QmiResult<()>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        $crate::__macro_support::null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

#[allow(unused)]
unsafe fn dummy_result<T>(_output: *mut T, _error: *mut *mut GError) -> Result<(), ()> {
    Ok(())
}

/// Generates an asynchronous setter using a custom input writer.
///
/// The supplied writer is responsible for populating the QMI input message
/// from the method parameter. The generated method performs the request,
/// validates the QMI response, and returns `QmiResult<()>` on completion.
///
/// This macro is a simplified wrapper around `setter_with_writer_extended!`,
/// using `()` as the return type and no additional output extraction.
macro_rules! setter_with_writer {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $writer:path,
        $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        setter_with_writer_extended!(
            $(#[$meta])*
            $method_name,
            $writer,
            $parameter_type,
            $message_input_new,
            $message_input_unref,
            $validation_start,
            $validation_finish,
            $message_output_get_result,
            $message_output_unref,
            (),
            crate::dummy_result,
            $context
        );
    }
}

/// Extended variant of `setter_with_writer!`.
///
/// In addition to writing the input parameters, this macro accepts a return
/// type and invokes a final extraction function on the successful QMI output
/// to produce the value returned by the generated method.
macro_rules! setter_with_writer_extended {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $writer:path,
        $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $return_type:ty,
        $return_function:path,
        $context:literal
    ) => {
        impl Context {
            $(#[$meta])*
            pub async fn $method_name(&self, parameter: $parameter_type) -> QmiResult<$return_type> {
                unsafe extern "C" fn callback(
                    client: *mut $crate::__macro_support::GObject,
                    res: *mut $crate::__macro_support::GAsyncResult,
                    user_data: *mut $crate::__macro_support::c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<$crate::__macro_support::oneshot::Sender<QmiResult<$return_type>>>());

                    let mut error: *mut GError = $crate::__macro_support::null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        $crate::__macro_support::GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => {
                            let mut error: *mut GError = $crate::__macro_support::null_mut();
                            $return_function(output_ptr, &mut error)
                                .map_err(|_| (error, concat!($context, " cb return function")).into())
                        }
                    };
                    $message_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { $message_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = $crate::__macro_support::null_mut();
                let ret = unsafe { $writer(input, &mut error, parameter) };
                if ret == $crate::__macro_support::GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = $crate::__macro_support::oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        $crate::__macro_support::null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

pub mod dms_client;
pub mod nas_client;
pub mod pds_client;
pub mod qmi_device;
pub mod uim_client;
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
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
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
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

unsafe fn c_char_to_string(value: *const std::ffi::c_char) -> String {
    std::ffi::CStr::from_ptr(value)
        .to_string_lossy()
        .into_owned()
}
