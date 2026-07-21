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
pub struct VoiceClient {
    client: NonNull<QmiClientVoice>,
    device: Device,
}

impl Drop for VoiceClient {
    fn drop(&mut self) {
        tracing::trace!("VOICE CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr() as _);
        }
    }
}

impl Clone for VoiceClient {
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

impl VoiceClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    /// Asynchronously allocates a new [`VoiceClient`].
    pub async fn new(device: &Device) -> QmiResult<VoiceClient> {
        let (tx, rx) = oneshot::channel::<QmiResult<NonNull<QmiClientVoice>>>();

        unsafe {
            qmi_device_allocate_client(
                device.as_ptr(),
                Service::Voice.into_glib(),
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

    /// Asynchronously releases the [`VoiceClient`] from the [`Device`].
    pub async fn close(self) -> QmiResult<()> {
        let ref_count = unsafe { *(self.client.as_ptr() as *mut GObject) }.ref_count;
        if ref_count != 2 {
            Err(QmiError::new(
                format!("can't close ref count = {}", ref_count),
                "voice close",
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

unsafe extern "C" fn allocate_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender =
        Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<NonNull<QmiClientVoice>>>);
    let mut error: *mut GError = null_mut();

    let client = qmi_device_allocate_client_finish(device as *mut QmiDevice, res, &mut error);

    let output = match NonNull::new(client as _) {
        None => Err((error, "new voice client").into()),
        Some(client) => Ok(client),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn release_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>);
    let mut error: *mut GError = null_mut();

    let ret = qmi_device_release_client_finish(device as *mut QmiDevice, res, &mut error);

    let output = match ret {
        GFALSE => Err((error, "release voice client").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
