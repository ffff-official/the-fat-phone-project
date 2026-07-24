use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::GFALSE;
use glib_sys::g_array_new;
use glib_sys::g_array_unref;
use glib_sys::guint8;
use gobject_sys::GObject;
use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElement;
use libqmi_glib_types_rs::UimPinId;
use libqmi_glib_types_rs::UimSessionType;
use libqmi_glib_types_rs::glib::translate::IntoGlib;

use std::cell::RefCell;
use std::ffi::CString;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::rc::Rc;

type CardStatusOutputElem = MessageUimGetCardStatusOutputCardStatusCardsElement;
type QmiCardStatusOutputElem = QmiMessageUimGetCardStatusOutputCardStatusCardsElement;

/// A client for interacting with the User Identity Module (UIM) interface of a QMI device.
///
/// [`UimClient`] provides asynchronous operations for accessing and managing
/// User Identity Modules through a QMI-compatible modem.
///
/// It can be used to:
///
/// - retrieve the status of available UIM cards;
/// - inspect the applications exposed by a card, such as SIM, USIM, ISIM, or
///   CSIM applications;
/// - query PIN, PUK, and personalization lock states;
/// - verify, change, enable, disable, or unblock PIN codes;
/// - access files stored in the UIM file system;
/// - receive notifications when the card state changes.
///
/// A modem may expose multiple cards, slots, or applications. Card-level
/// information, such as the universal PIN state, is reported separately from
/// application-specific information, such as PIN1, PIN2, and the application
/// identifier.
///
/// The underlying QMI client is reference-counted and remains valid while at
/// least one [`UimClient`] instance exists.
#[derive(Debug)]
pub struct UimClient {
    client: NonNull<QmiClientUim>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(UimClient);

client_clone_impl!(UimClient);

impl UimClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(UimClient, QmiClientUim, libqmi_glib_types_rs::Service::Uim);

    close_client_impl!("uim");

    /// Retrieves the current status of all UIM cards.
    ///
    /// This operation queries the modem for the state of every available UIM card
    /// and the applications they expose. The returned information includes:
    ///
    /// - the overall card state;
    /// - the universal PIN (UPIN) state and remaining retry counters;
    /// - any card-level error;
    /// - the list of applications available on each card, including their
    ///   application type, state, personalization status, PIN1/PIN2 states,
    ///   retry counters, and application identifier (AID).
    ///
    /// Most devices expose a single card, but multi-SIM devices may return
    /// multiple entries, one for each card or slot.
    ///
    /// # Returns
    ///
    /// On success, returns a vector describing the status of every detected UIM
    /// card.
    ///
    /// # Errors
    ///
    /// Returns a [`QmiError`] if the request fails or if the modem reports an
    /// error while retrieving the card status.
    pub async fn get_card_status(&self) -> QmiResult<Vec<CardStatusOutputElem>> {
        let (tx, rx) = oneshot::channel::<QmiResult<Vec<CardStatusOutputElem>>>();

        unsafe {
            qmi_client_uim_get_card_status(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_card_status_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
        }

        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Verifies the PIN1 code of the selected UIM application.
    ///
    /// On success, returns the number of remaining PIN verification attempts.
    ///
    /// # Errors
    ///
    /// Returns a [`QmiError`] if the PIN is invalid or the request fails.
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
                            Box::into_raw(Box::new(tx)).cast(),
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

allocate_client_callback!(allocate_client_cb, QmiClientUim, "allocate UIM client");

release_client_callback!(release_client_cb, "release UIM client");

unsafe extern "C" fn get_card_status_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender =
        Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<Vec<CardStatusOutputElem>>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_uim_get_card_status_finish(client.cast(), res, &mut error);
    let output = match output.is_null() {
        true => Err((error, "get card status cb finish").into()),
        false => {
            let ret = qmi_message_uim_get_card_status_output_get_result(output, &mut error);
            match ret {
                GFALSE => {
                    qmi_message_uim_get_card_status_output_unref(output);
                    Err((error, "get card status get result").into())
                }
                _ => {
                    let mut value_card_status_index_gw_primary = 0;
                    let mut value_card_status_index_1x_primary = 0;
                    let mut value_card_status_index_gw_secondary = 0;
                    let mut value_card_status_index_1x_secondary = 0;
                    let mut cards: *mut glib::GArray = null_mut();
                    let ret = qmi_message_uim_get_card_status_output_get_card_status(
                        output,
                        &mut value_card_status_index_gw_primary,
                        &mut value_card_status_index_1x_primary,
                        &mut value_card_status_index_gw_secondary,
                        &mut value_card_status_index_1x_secondary,
                        &mut cards,
                        &mut error,
                    );
                    match ret {
                        GFALSE => {
                            qmi_message_uim_get_card_status_output_unref(output);
                            Err((error, "get card status output get card status").into())
                        }
                        _ => {
                            let slice = std::slice::from_raw_parts(
                                (*cards).data.cast::<QmiCardStatusOutputElem>(),
                                (*cards).len as usize,
                            );
                            let final_output = slice
                                .iter()
                                .map(|card| CardStatusOutputElem::from(*card))
                                .collect::<Vec<_>>();

                            qmi_message_uim_get_card_status_output_unref(output);

                            Ok(final_output)
                        }
                    }
                }
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn verify_pin_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<u8>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_uim_verify_pin_finish(client.cast(), res, &mut error);
    let output = match output.is_null() {
        true => Err((error, "verify pin cb finish").into()),
        false => {
            let ret = { qmi_message_uim_verify_pin_output_get_result(output, &mut error) };
            qmi_message_uim_verify_pin_output_unref(output);
            match ret {
                GFALSE => Err((error, "verify pin output").into()),
                _ => Ok(0),
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
