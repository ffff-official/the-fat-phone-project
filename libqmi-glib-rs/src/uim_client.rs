use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::GFALSE;
use glib_sys::g_array_new;
use glib_sys::g_array_unref;
use glib_sys::guint8;
use gobject_sys::{GObject, g_object_ref, g_object_unref};
use libqmi_glib_types_rs::DeviceReleaseClientFlags;
use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElement;
use libqmi_glib_types_rs::Service;
use libqmi_glib_types_rs::UimPinId;
use libqmi_glib_types_rs::UimSessionType;
use libqmi_glib_types_rs::glib::translate::IntoGlib;

use std::ffi::CString;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::ptr::null_mut;

#[derive(Debug)]
pub struct UimClient {
    client: NonNull<QmiClientUim>,
    device: Device,
}

impl Drop for UimClient {
    fn drop(&mut self) {
        tracing::trace!("UIM CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr() as _);
        }
    }
}

impl Clone for UimClient {
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

impl UimClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    /// Asynchronously allocates a new [`UimClient`].
    pub async fn new(device: &Device) -> QmiResult<UimClient> {
        let (tx, rx) = oneshot::channel::<QmiResult<NonNull<QmiClientUim>>>();

        unsafe {
            qmi_device_allocate_client(
                device.as_ptr(),
                Service::Uim.into_glib(),
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

    /// Asynchronously releases the [`UimClient`] from the [`Device`].
    pub async fn close(self) -> QmiResult<()> {
        let ref_count = unsafe { *(self.client.as_ptr() as *mut GObject) }.ref_count;
        if ref_count != 2 {
            Err(QmiError::new(
                format!("can't close ref count = {}", ref_count),
                "uim close",
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

    pub async fn get_card_status(
        &self,
    ) -> QmiResult<Vec<MessageUimGetCardStatusOutputCardStatusCardsElement>> {
        let (tx, rx) = oneshot::channel::<
            QmiResult<Vec<MessageUimGetCardStatusOutputCardStatusCardsElement>>,
        >();

        unsafe {
            qmi_client_uim_get_card_status(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_card_status_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            );
        }

        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    pub async fn verify_pin(&self, pin_code: impl AsRef<str>) -> QmiResult<u8> {
        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let c_string = CString::new(pin_code.as_ref())
            .map_err(|_e| QmiError::new("Trailing NULL value".to_string(), "verify_pin"))?;

        let input = unsafe { qmi_message_uim_verify_pin_input_new() };

        let mut error: *mut GError = null_mut();
        let ret = unsafe {
            qmi_message_uim_verify_pin_input_set_info(
                input,
                UimPinId::Pin1.into_glib(),
                c_string.as_ptr(),
                &mut error,
            )
        };
        match ret {
            GFALSE => unsafe {
                qmi_message_uim_verify_pin_input_unref(input);
                return Err((error, "verify pin").into());
            },
            _ => unsafe {
                let placeholder_aid = g_array_new(GFALSE, GFALSE, size_of::<guint8>() as _);
                let ret = qmi_message_uim_verify_pin_input_set_session(
                    input,
                    UimSessionType::CardSlot1.into_glib(),
                    placeholder_aid,
                    &mut error,
                );
                g_array_unref(placeholder_aid);
                match ret {
                    GFALSE => {
                        qmi_message_uim_verify_pin_input_unref(input);
                        return Err((error, "verify pin set session").into());
                    }
                    _ => {
                        qmi_client_uim_verify_pin(
                            self.client.as_ptr(),
                            input,
                            OPERATION_TIMEOUT,
                            null_mut(),
                            Some(verify_pin_cb),
                            Box::into_raw(Box::new(tx)) as *mut c_void,
                        );
                    }
                }
            },
        }

        unsafe {
            qmi_message_uim_verify_pin_input_unref(input);
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
        Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<NonNull<QmiClientUim>>>)
    };
    let mut error: *mut GError = null_mut();

    let client =
        unsafe { qmi_device_allocate_client_finish(device as *mut QmiDevice, res, &mut error) };

    let output = match NonNull::new(client as _) {
        None => Err((error, "new uim client").into()),
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
        GFALSE => Err((error, "release uim client").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
extern "C" fn get_card_status_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = unsafe {
        Box::from_raw(
            user_data
                as *mut oneshot::Sender<
                    QmiResult<Vec<MessageUimGetCardStatusOutputCardStatusCardsElement>>,
                >,
        )
    };
    let mut error: *mut GError = null_mut();

    let output = unsafe { qmi_client_uim_get_card_status_finish(device as _, res, &mut error) };
    let output = match output.is_null() {
        true => Err((error, "get card status cb finish").into()),
        false => {
            let ret =
                unsafe { qmi_message_uim_get_card_status_output_get_result(output, &mut error) };
            match ret {
                GFALSE => {
                    unsafe {
                        qmi_message_uim_get_card_status_output_unref(output);
                    }
                    Err((error, "get card status get result").into())
                }
                _ => {
                    let mut value_card_status_index_gw_primary = 0;
                    let mut value_card_status_index_1x_primary = 0;
                    let mut value_card_status_index_gw_secondary = 0;
                    let mut value_card_status_index_1x_secondary = 0;
                    let mut cards: *mut glib::GArray = null_mut();
                    let ret = unsafe {
                        qmi_message_uim_get_card_status_output_get_card_status(
                            output,
                            &mut value_card_status_index_gw_primary,
                            &mut value_card_status_index_1x_primary,
                            &mut value_card_status_index_gw_secondary,
                            &mut value_card_status_index_1x_secondary,
                            &mut cards,
                            &mut error,
                        )
                    };
                    match ret {
                        GFALSE => {
                            unsafe {
                                qmi_message_uim_get_card_status_output_unref(output);
                            }
                            Err((error, "get card status output get card status").into())
                        }
                        _ => {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                (*cards).data as *const QmiMessageUimGetCardStatusOutputCardStatusCardsElement,
                                (*cards).len as usize,
                            )
                            };
                            let final_output = slice
                                .iter()
                                .map(|card| {
                                    MessageUimGetCardStatusOutputCardStatusCardsElement::from(*card)
                                })
                                .collect::<Vec<_>>();
                            unsafe {
                                qmi_message_uim_get_card_status_output_unref(output);
                            }
                            Ok(final_output)
                        }
                    }
                }
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
extern "C" fn verify_pin_cb(device: *mut GObject, res: *mut GAsyncResult, user_data: *mut c_void) {
    let sender = unsafe { Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<u8>>) };
    let mut error: *mut GError = null_mut();

    let output = unsafe { qmi_client_uim_verify_pin_finish(device as _, res, &mut error) };
    let output = match output.is_null() {
        true => Err((error, "verify pin cb finish").into()),
        false => unsafe {
            let ret = { qmi_message_uim_verify_pin_output_get_result(output, &mut error) };
            qmi_message_uim_verify_pin_output_unref(output);
            match ret {
                GFALSE => Err((error, "verify pin output").into()),
                _ => Ok(0),
            }
        },
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
