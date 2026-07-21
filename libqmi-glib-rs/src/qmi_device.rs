use super::*;

use crate::sys::*;
use futures::channel::oneshot;
use gio_sys::{GAsyncResult, GFile};
use glib_sys::{GError, GFALSE};
use gobject_sys::{GObject, g_object_ref, g_object_unref};
use libqmi_glib_types_rs::glib::translate::FromGlib;
use libqmi_glib_types_rs::glib::translate::IntoGlib;
use libqmi_glib_types_rs::{DeviceExpectedDataFormat, DeviceOpenFlags};

use std::ffi::{CStr, c_void};
use std::ops::Deref;
use std::ptr::{NonNull, null_mut};

/// Represents a QMI device and provides the entry point for creating clients and communicating with its services.
#[derive(Debug)]
pub struct Device(NonNull<QmiDevice>);

impl Drop for Device {
    fn drop(&mut self) {
        tracing::debug!("QMI DEVICE is droped !");
        unsafe {
            g_object_unref(self.0.as_ptr() as _);
        }
    }
}

impl Clone for Device {
    fn clone(&self) -> Self {
        unsafe {
            g_object_ref(self.0.as_ptr() as _);
        }
        Self(self.0)
    }
}

impl Deref for Device {
    type Target = NonNull<QmiDevice>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Device {
    /// Asynchronously creates a [`Device`] object to manage *file*.
    pub async fn new(file: *mut GFile) -> QmiResult<Self> {
        let (tx, rx) = oneshot::channel::<QmiResult<Self>>();

        unsafe {
            qmi_device_new(
                file,
                null_mut(),
                Some(device_new_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            )
        };
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously opens a [`Device`] for I/O.
    pub async fn open(&mut self) -> QmiResult<()> {
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

        unsafe {
            qmi_device_open(
                self.0.as_ptr(),
                (DeviceOpenFlags::NONE | DeviceOpenFlags::AUTO).into_glib(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(device_open_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            )
        };
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously closes a [`Device`], preventing any further I/O.
    pub async fn close(self) -> QmiResult<()> {
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

        unsafe {
            qmi_device_close_async(
                self.0.as_ptr(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(device_close_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            );
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Get the WWAN interface name associated with the QMI control port.
    pub fn get_wwan_iface(&self) -> String {
        let s = unsafe { CStr::from_ptr(qmi_device_get_wwan_iface(self.0.as_ptr())) };
        s.to_string_lossy().into()
    }

    /// Retrieves the data format currently expected by the kernel in the network interface.
    pub fn get_expected_data_format(&self) -> QmiResult<DeviceExpectedDataFormat> {
        let mut error: *mut GError = null_mut();

        let output = unsafe {
            DeviceExpectedDataFormat::from_glib(qmi_device_get_expected_data_format(
                self.0.as_ptr(),
                &mut error,
            ))
        };
        if error.is_null() {
            Ok(output)
        } else {
            Err((error, "open device").into())
        }
    }

    /// Configures the data format currently expected by the kernel in the network interface.
    pub fn set_expected_data_fornnat(
        &self,
        data_format: DeviceExpectedDataFormat,
    ) -> QmiResult<()> {
        let mut error: *mut GError = null_mut();

        let ret = unsafe {
            qmi_device_set_expected_data_format(
                self.0.as_ptr(),
                data_format.into_glib(),
                &mut error,
            )
        };
        match ret {
            GFALSE => Err((error, "open device").into()),
            _ => Ok(()),
        }
    }
}

unsafe extern "C" fn device_new_cb(
    _unused: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<Device>>);

    let mut error: *mut GError = null_mut();

    let device = qmi_device_new_finish(res, &mut error);

    let output = match NonNull::new(device) {
        None => Err((error, "new device").into()),
        Some(device) => Ok(Device(device)),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn device_open_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>);

    let mut error: *mut GError = null_mut();

    let ret = qmi_device_open_finish(device as *mut QmiDevice, res, &mut error);

    let output = match ret {
        GFALSE => Err((error, "open device").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn device_close_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>);

    let mut error: *mut GError = null_mut();

    let ret = qmi_device_close_finish(device as *mut QmiDevice, res, &mut error);

    let output = match ret {
        GFALSE => Err((error, "close device").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
