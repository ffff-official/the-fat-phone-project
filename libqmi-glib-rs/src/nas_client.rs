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

/// A client for interacting with the Network Access Service (NAS) of a QMI device.
#[derive(Debug)]
pub struct NasClient {
    client: NonNull<QmiClientNas>,
    device: Device,
}

impl Drop for NasClient {
    fn drop(&mut self) {
        tracing::trace!("NAS CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr().cast());
        }
    }
}

impl Clone for NasClient {
    fn clone(&self) -> Self {
        unsafe {
            g_object_ref(self.client.as_ptr().cast());
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
                Box::into_raw(Box::new(tx)).cast(),
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
        let ref_count = unsafe { *(self.client.as_ptr().cast::<GObject>()) }.ref_count;
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
                self.client.as_ptr().cast(),
                DeviceReleaseClientFlags::RELEASE_CID.into_glib(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(release_client_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }
}

allocate_client_callback!(allocate_client_cb, QmiClientNas, "allocate NAS client");

release_client_callback!(release_client_cb, "release NAS client");
