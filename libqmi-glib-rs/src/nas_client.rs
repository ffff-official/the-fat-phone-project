use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::GFALSE;
use gobject_sys::{GObject, g_object_ref, g_object_unref};
use libqmi_glib_types_rs::DeviceReleaseClientFlags;
use libqmi_glib_types_rs::Service;
use libqmi_glib_types_rs::glib::translate::IntoGlib;

use std::ffi::c_void;
use std::ptr::NonNull;
use std::ptr::null_mut;

#[derive(Debug)]
pub struct NasClient {
    client: NonNull<QmiClientNas>,
    device: Device,
}

impl Drop for NasClient {
    fn drop(&mut self) {
        tracing::trace!("NAS CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr() as _);
        }
    }
}

impl Clone for NasClient {
    fn clone(&self) -> Self {
        unsafe {
            g_object_ref(self.client.as_ptr() as _);
        }
        Self {
            client: self.client,
            device: self.device.clone(),
        }
    }
}

impl NasClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    /// Asynchronously allocates a new [`NasClient`].
    pub async fn new(device: &Device) -> QmiResult<NasClient> {
        let (tx, rx) = oneshot::channel::<QmiResult<NonNull<QmiClientNas>>>();

        unsafe {
            qmi_device_allocate_client(
                device.as_ptr(),
                Service::Nas.into_glib(),
                QMI_CID_NONE as _,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(allocate_client_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            )
        };
        match rx.await.map_err(|_| QmiError::cb_droped())? {
            Ok(client) => Ok(Self {
                device: device.clone(),
                client,
            }),
            Err(e) => Err(e),
        }
    }

    /// Asynchronously releases the [`NasClient`] from the [`Device`].
    pub async fn close(self) -> QmiResult<()> {
        let ref_count = unsafe { *(self.client.as_ptr() as *mut GObject) }.ref_count;
        if ref_count != 2 {
            Err(QmiError::new(
                format!("can't close ref count = {}", ref_count),
                "Nas close",
            ))?;
        }
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

        unsafe {
            qmi_device_release_client(
                self.device.as_ptr(),
                self.client.as_ptr() as _,
                DeviceReleaseClientFlags::RELEASE_CID.into_glib(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(release_client_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            );
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }
}

extern "C" fn allocate_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = unsafe {
        Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<NonNull<QmiClientNas>>>)
    };
    let mut error: *mut GError = null_mut();

    let client =
        unsafe { qmi_device_allocate_client_finish(device as *mut QmiDevice, res, &mut error) };

    let output = match NonNull::new(client as _) {
        None => Err((error, "new nas client").into()),
        Some(client) => Ok(client),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
extern "C" fn release_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = unsafe { Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>) };
    let mut error: *mut GError = null_mut();

    let ret =
        unsafe { qmi_device_release_client_finish(device as *mut QmiDevice, res, &mut error) };

    let output = match ret {
        GFALSE => Err((error, "release nas client").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
