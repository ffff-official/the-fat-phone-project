//! **libqmi-glib-rs provides safe, asynchronous Rust bindings for libqmi-glib.**
//!
//! It allows applications to communicate with QMI-based cellular modems using
//! idiomatic async/await while leveraging GLib's asynchronous APIs under the
//! hood.
//!
//! The library exposes a safe, high-level API covering a substantial subset of
//! libqmi-glib services, including:
//!
//! - **Device management**, including device creation and lifecycle handling.
//! - **WDS (Wireless Data Service)** for packet data connections and network
//!   configuration.
//! - **WMS (Wireless Messaging Service)** for sending SMS messages, receiving
//!   SMS indications, and managing message routes.
//! - **NAS (Network Access Service)** for network registration, signal quality,
//!   system selection preferences, and radio information.
//! - **DMS (Device Management Service)** for querying modem capabilities and
//!   device information.
//! - **VOICE** for placing, answering, terminating, and monitoring voice calls
//!   through asynchronous call status indications.
//! - **UIM (User Identity Module)** for selected SIM card management
//!   operations.
//! - **PDS (Position Determination Service)** for selected GNSS/GPS
//!   configuration features.
//!
//! The API follows Rust idioms while remaining close to the underlying QMI
//! protocol. Most QMI requests are exposed as asynchronous methods, and common
//! request patterns are implemented through reusable abstractions, providing a
//! consistent interface across services.
//! ```no_run
//! use libqmi_glib_rs::*;
//!
//! use glib::MainLoop;
//!
//! fn main() {
//!     let ctx = glib::MainContext::default();
//!     let main_loop = MainLoop::new(Some(&ctx), false);
//!     let main_loop_clone = main_loop.clone();
//!
//!     ctx.spawn_local(async move {
//!         let mut device = Device::new("/dev/cdc-wdm1").await?;
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
//!     main_loop.run();
//! }
//! ```
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(unsafe_op_in_unsafe_fn)] // Warnings on unsafe_op_in_unsafe_fn is a shitty idea for a crate of this kind.

#[doc(hidden)]
pub(crate) mod __macro_support {
    pub(crate) use crate::sys::*;
    pub use futures::channel::oneshot;
    pub use gio_sys::GAsyncResult;
    pub use glib_sys::GFALSE;
    pub use gobject_sys::{
        GClosure, GObject, g_object_ref, g_object_unref, g_signal_connect_data,
        g_signal_handler_disconnect,
    };
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
            use $crate::__macro_support::*;
            let (tx, rx) = oneshot::channel::<QmiResult<NonNull<$ffi_client_type>>>();

            unsafe {
                qmi_device_allocate_client(
                    device.as_ptr(),
                    IntoGlib::into_glib($service),
                    QMI_CID_NONE as _,
                    $crate::OPERATION_TIMEOUT,
                    null_mut(),
                    Some(allocate_client_cb),
                    Box::into_raw(Box::new(tx)).cast(),
                )
            };
            match rx.await.map_err(|_| QmiError::cb_droped("new client"))? {
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
            use $crate::__macro_support::*;
            let ref_count = unsafe { *(self.client.as_ptr().cast::<GObject>()) }.ref_count;
            if ref_count != 2 {
                Err(QmiError::new(
                    format!("can't close ref count = {}", ref_count),
                    concat!($context, " close"),
                ))?;
            }
            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            unsafe {
                qmi_device_release_client(
                    self.device.as_ptr(),
                    self.client.as_ptr().cast(),
                    DeviceReleaseClientFlags::RELEASE_CID.into_glib(),
                    $crate::OPERATION_TIMEOUT,
                    null_mut(),
                    Some(release_client_cb),
                    Box::into_raw(Box::new(tx)).cast(),
                );
            }
            rx.await.map_err(|_| QmiError::cb_droped("close client"))?
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
            use $crate::__macro_support::*;
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
            device: *mut $crate::__macro_support::GObject,
            res: *mut $crate::__macro_support::GAsyncResult,
            user_data: *mut $crate::__macro_support::c_void,
        ) {
            use $crate::__macro_support::*;
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
                use $crate::__macro_support::*;
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == GFALSE {
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
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(from_glib(flags))
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        null_mut(),
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped($context))?
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
                use $crate::__macro_support::*;
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == GFALSE {
                        $message_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut error: *mut GError = null_mut();
                    let mut final_result = <$return_type>::default();
                    let ret = $extractor(output, &mut error, &mut final_result);

                    $message_output_unref(output);
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(final_result)
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        null_mut(),
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped($context))?
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
        $parameter_name:ident: $parameter_type:ty,
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
            pub async fn $method_name(&self, $parameter_name: $parameter_type) -> QmiResult<()> {
                use $crate::__macro_support::*;
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    $message_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { $message_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = null_mut();

                use IntoGlib;
                let ret = unsafe { $qmi_function(input, $parameter_name.into_glib(), &mut error) };
                if ret == GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped($context))?
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
        $parameter_name:ident: $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        request_with_writer_and_extractor!(
            $(#[$meta])*
            $method_name,
            $writer,
            $parameter_name: $parameter_type,
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
macro_rules! request_with_writer_and_extractor {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $writer:path,
        $parameter_name:ident: $parameter_type:ty,
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
            pub async fn $method_name(&self, $parameter_name: $parameter_type) -> QmiResult<$return_type> {
                use $crate::__macro_support::*;
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<$return_type>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => {
                            let mut error: *mut GError = null_mut();
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

                let mut error: *mut GError = null_mut();
                let ret = unsafe { $writer(input, &mut error, $parameter_name) };
                if ret == GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped($context))?
            }
        }
    }
}

/// Generates an asynchronous event registration method for a QMI indication.
///
/// The generated method performs the following steps:
/// - Connects a GLib signal handler to the underlying QMI client.
/// - Configures the indication by sending the corresponding QMI request.
/// - Keeps the user callback alive until the signal handler is disconnected or the underlying GObject is destroyed.
///
/// Unlike request-oriented macros such as getter! or setter_with_writer!,
/// this macro does not generate the indication callback itself. The callback
/// responsible for decoding the indication payload and invoking the user
/// callback must be provided separately.
///
/// # Parameters
///
/// - $method_name — Name of the generated method.
/// - $callback_type — Type of the user callback.
/// - $parameter_name + $parameter_type — Given for initialization.
/// - $signal_name — GLib signal associated with the indication (for example c"event-report").
/// - $callback_input — QMI indication output type received by the signal.
/// - $callback_function — User-defined indication callback.
/// - $writer — Function configuring the indication input message.
/// - $message_input_new — Allocates the QMI input message.
/// - $message_input_unref — Releases the QMI input message.
/// - $validation_start — Starts the asynchronous QMI request.
/// - $validation_finish — Finishes the asynchronous QMI request.
/// - $message_output_get_result — Validates the QMI response.
/// - $message_output_unref — Releases the QMI output message.
/// - $context — Context string used in generated error messages.
///
/// # Generated method
///
/// The generated method returns QmiResult<()>. Once it completes
/// successfully, the indication remains registered until the signal handler is
/// disconnected or the underlying GObject is destroyed.
macro_rules! event {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $callback_type:ty,
        $parameter_name:ident: $parameter_type:ty,
        $signal_name:expr,
        $callback_input:ty,
        $callback_function:path,
        $writer:path,
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
            pub async fn $method_name(&mut self, cb: $callback_type, $parameter_name: $parameter_type) -> QmiResult<()> {
                use $crate::__macro_support::*;
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
                    let mut error: *mut GError = null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        let _drop = sender
                            .send(Err((error, concat!($context, " finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    let result: Result<(), _> = match ret {
                        GFALSE => Err((error, concat!($context, " get result")).into()),
                        _ => Ok(()),
                    };
                    $message_output_unref(output);
                    drop(sender.send(result).inspect_err(cannot_send));
                }
                // Called when the signal handler is disconnected or when the underlying GObject is destroyed.
                unsafe extern "C" fn destroyer(
                    data: *mut c_void,
                    _closure: *mut GClosure
                ) {
                    tracing::debug!(concat!("destroy : ", $context));
                    drop(Box::from_raw(data.cast::<$callback_type>()));
                }
                unsafe {
                    let cb_ptr = Box::into_raw(Box::new(cb)).cast();
                    let handler_id = g_signal_connect_data(
                        self.client.as_ptr().cast(),
                        $signal_name.as_ptr(),
                        Some(std::mem::transmute::<
                            unsafe extern "C" fn(
                                *mut GObject,
                                *mut $callback_input,
                                *mut c_void,
                            ),
                            unsafe extern "C" fn(),
                        >($callback_function)),
                        cb_ptr,
                        Some(destroyer),
                        0,
                    );
                    if handler_id == 0 {
                        drop(Box::from_raw(cb_ptr.cast::<$callback_type>()));
                        Err(QmiError::new("handler id = 0", $context))?;
                    } else {
                        tracing::debug!(concat!($context, " handler id = {}"), handler_id);
                    }

                    let input = $message_input_new();
                    if input.is_null() {
                        g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                        return Err(QmiError::new("Null input pointer", $context));
                    }

                    let mut error: *mut GError = null_mut();

                    let ret = $writer(input, &mut error, $parameter_name);
                    if ret == GFALSE {
                        $message_input_unref(input);
                        g_signal_handler_disconnect(
                            self.client.as_ptr().cast(),
                            handler_id,
                        );
                        Err((error, $context))?;
                    }

                    let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                    $message_input_unref(input);

                    let result = match rx.await {
                        Ok(result) => result,
                        Err(_) => {
                            g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                            return Err(QmiError::cb_droped($context));
                        }
                    };

                    if result.is_err() {
                        g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                    } else {
                        tracing::debug!(
                            concat!($context, " waiting for messages...")
                        );
                    }
                    result
                }
            }
        }
    }
}

pub mod device;
pub mod dms;
pub mod nas;
pub mod pds;
pub mod uim;
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
pub mod voice;
pub mod wds;
pub mod wms;

/// Re-exports all definitions from libqmi-glib-sys.
pub(crate) mod sys {
    pub(crate) use libqmi_glib_sys::*;
}
/// Re-exports all definitions from libqmi-glib-types-rs.
pub mod types {
    pub use libqmi_glib_types_rs::*;
}

use std::fmt::Debug;

#[doc(inline)]
pub use device::Device;
#[doc(inline)]
pub use dms::DmsClient;
#[doc(inline)]
pub use nas::NasClient;
#[doc(inline)]
pub use pds::PdsClient;
#[doc(inline)]
pub use uim::UimClient;
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
#[doc(inline)]
pub use voice::VoiceClient;
#[doc(inline)]
pub use wds::WdsClient;
#[doc(inline)]
pub use wms::WmsClient;

use glib_sys::{GError, g_clear_error};

const OPERATION_TIMEOUT: u32 = 5;

fn cannot_send<T: std::fmt::Debug>(result: &QmiResult<T>) {
    tracing::error!("Cannot send : {:?}", result);
}

pub type QmiResult<T> = Result<T, QmiError>;

/// Error type returned by the library.
///
/// `QmiError` combines the human-readable error message reported by GLib with
/// additional information identifying the origin of the failure.
///
/// When available, the original GLib error domain and error code are preserved.
/// For QMI-specific errors, these values are used by the `Debug` implementation
/// to display the corresponding `ProtocolError` or `CoreError` variant.
///
/// The `context` field identifies the operation that produced the error,
/// making it easier to locate failures when debugging.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct QmiError {
    /// Human-readable error message.
    pub message: String,
    /// Name of the operation that produced the error.
    pub context: &'static str,
    /// Numeric error code reported by GLib, if available.
    pub code: Option<i32>,
    /// GLib error domain (`GQuark`), if available.
    pub domain: Option<u32>,
}

impl Debug for QmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use libqmi_glib_sys::{qmi_core_error_quark, qmi_protocol_error_quark};
        use libqmi_glib_types_rs::glib::translate::FromGlib;
        use libqmi_glib_types_rs::{CoreError, ProtocolError};

        if let (Some(domain), Some(code)) = (self.domain, self.code) {
            unsafe {
                let domain_name =
                    std::ffi::CStr::from_ptr(glib_sys::g_quark_to_string(domain)).to_string_lossy();
                if domain == qmi_protocol_error_quark() {
                    let protocol_error = ProtocolError::from_glib(code);
                    write!(
                        f,
                        "{}: [QMI_PROTOCOL_ERROR: {:?} ({})]",
                        self.context, protocol_error, code
                    )
                } else if domain == qmi_core_error_quark() {
                    let core_error = CoreError::from_glib(code);
                    write!(
                        f,
                        "{}: [QMI_CORE_ERROR: {:?} ({})]",
                        self.context, core_error, code
                    )
                } else {
                    write!(
                        f,
                        "{}: {} [{} ({})]",
                        self.context, self.message, domain_name, code
                    )
                }
            }
        } else {
            write!(f, "{}: {}", self.context, self.message)
        }
    }
}

#[doc(hidden)]
impl From<(*mut GError, &'static str)> for QmiError {
    fn from((mut error, context): (*mut GError, &'static str)) -> Self {
        if error.is_null() {
            return Self {
                message: "Unknown GLib error".into(),
                context,
                code: None,
                domain: None,
            };
        }

        unsafe {
            let ret = Self {
                message: std::ffi::CStr::from_ptr((*error).message)
                    .to_string_lossy()
                    .into_owned(),
                context,
                code: Some((*error).code),
                domain: Some((*error).domain),
            };

            g_clear_error(&mut error);
            ret
        }
    }
}

impl std::fmt::Display for QmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

impl QmiError {
    fn new(message: impl AsRef<str>, context: &'static str) -> Self {
        Self {
            message: message.as_ref().to_string(),
            context,
            code: None,
            domain: None,
        }
    }
}

impl std::error::Error for QmiError {}

#[doc(hidden)]
impl<T: std::error::Error> From<(T, &'static str, ())> for QmiError {
    fn from((error, context, _): (T, &'static str, ())) -> Self {
        Self {
            message: error.to_string(),
            context,
            code: None,
            domain: None,
        }
    }
}

impl QmiError {
    fn cb_droped(context: &'static str) -> Self {
        Self {
            message: "cb droped".to_string(),
            context,
            code: None,
            domain: None,
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
