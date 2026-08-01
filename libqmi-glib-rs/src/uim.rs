#[cfg(feature = "v1_10")]
pub use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElement as CardStatusOutputElem;

#[cfg(feature = "v1_22")]
pub use libqmi_glib_types_rs::{UimPinId, UimSessionType};

#[cfg(feature = "v1_26")]
pub use libqmi_glib_types_rs::{UimCardProtocol, UimPhysicalCardState, UimSlotState};

#[cfg(feature = "v1_10")]
pub type QmiCardStatusOutputElem = QmiMessageUimGetCardStatusOutputCardStatusCardsElement;

use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::GArray;
use glib_sys::GFALSE;
use glib_sys::GTRUE;
use glib_sys::g_array_new;
use glib_sys::g_array_unref;
use glib_sys::gboolean;
use glib_sys::guint8;
use gobject_sys::GObject;

use std::cell::RefCell;
use std::ffi::CString;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::rc::Rc;

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

    /// TODO: This is bullshit.
    /// This request does not fit the generic setter pattern because
    /// retries_remaining must be extracted from the output when get_result() fails.
    /// Keep this implementation explicit until the request abstraction supports
    /// error-output extraction.
    ///
    /// Remaining retries are available on failure with
    /// Must USE : qmi_message_uim_verify_pin_output_get_retries_remaining().
    ///
    /// Verifies the PIN1 code of the selected UIM application.
    ///
    /// On success, returns the number of remaining PIN verification attempts.
    ///
    /// # Errors
    ///
    /// Returns a [`QmiError`] if the PIN is invalid or the request fails.
    #[cfg(feature = "v1_22")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_22")))]
    pub async fn verify_pin(&self, pin_code: impl AsRef<str>) -> QmiResult<u8> {
        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let c_string = CString::new(pin_code.as_ref())
            .map_err(|_e| QmiError::new("Trailing NULL value", "verify_pin"))?;

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

type Context = UimClient;

#[cfg(feature = "v1_22")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_22")))]
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

#[cfg(feature = "v1_10")]
unsafe fn get_card_status_extractor(
    output: *mut QmiMessageUimGetCardStatusOutput,
    error: *mut *mut GError,
    param: &mut UimCardStatus,
) -> gboolean {
    let mut cards: *mut GArray = null_mut();

    let ret = qmi_message_uim_get_card_status_output_get_card_status(
        output,
        &mut param.index_gw_primary,
        &mut param.index_1x_primary,
        &mut param.index_gw_secondary,
        &mut param.index_1x_secondary,
        &mut cards,
        error,
    );

    if ret == GTRUE && !cards.is_null() && (*cards).len > 0 {
        let slice = std::slice::from_raw_parts(
            (*cards).data.cast::<QmiCardStatusOutputElem>(),
            (*cards).len as usize,
        );

        param.cards = slice
            .iter()
            .map(|card| CardStatusOutputElem::from(*card))
            .collect();
    }
    ret
}

#[cfg(feature = "v1_10")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_10")))]
/// Status information for the UIM cards and their applications.
///
/// The application indexes identify the primary and secondary GW (GSM/WCDMA)
/// and 1X (CDMA2000) applications reported by the device.
#[derive(Debug, Clone, Default)]
pub struct UimCardStatus {
    /// Index of the primary GW (GSM/WCDMA) application.
    pub index_gw_primary: u16,
    /// Index of the primary 1X (CDMA2000) application.
    pub index_1x_primary: u16,
    /// Index of the secondary GW (GSM/WCDMA) application.
    pub index_gw_secondary: u16,
    /// Index of the secondary 1X (CDMA2000) application.
    pub index_1x_secondary: u16,
    /// Status information for the UIM cards reported by the device.
    pub cards: Vec<CardStatusOutputElem>,
}

getter_with_extractor!(
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
    #[cfg(feature = "v1_10")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_10")))]
    get_card_status,
    get_card_status_extractor,
    UimCardStatus,
    qmi_client_uim_get_card_status,
    qmi_client_uim_get_card_status_finish,
    qmi_message_uim_get_card_status_output_get_result,
    qmi_message_uim_get_card_status_output_unref,
    "get_card_status"
);

#[cfg(feature = "v1_14")]
unsafe fn get_supported_messages_extractor(
    output: *mut QmiMessageUimGetSupportedMessagesOutput,
    error: *mut *mut GError,
    param: &mut Vec<u8>,
) -> gboolean {
    let mut value_list: *mut GArray = null_mut();

    let ret =
        qmi_message_uim_get_supported_messages_output_get_list(output, &mut value_list, error);

    if ret == GTRUE && !value_list.is_null() && (*value_list).len > 0 {
        let slice =
            std::slice::from_raw_parts((*value_list).data.cast::<u8>(), (*value_list).len as usize);

        *param = slice.to_vec();
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the list of UIM messages supported by the device.
    ///
    /// The returned byte vector contains a bitmap indicating which UIM
    /// message identifiers are supported by the modem.
    #[cfg(feature = "v1_14")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
    get_supported_messages,
    get_supported_messages_extractor,
    Vec<u8>,
    qmi_client_uim_get_supported_messages,
    qmi_client_uim_get_supported_messages_finish,
    qmi_message_uim_get_supported_messages_output_get_result,
    qmi_message_uim_get_supported_messages_output_unref,
    "get_supported_messages"
);

/// Status information for a physical UIM slot.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone)]
pub struct UimPhysicalSlotStatus {
    /// Physical card state reported for the slot.
    pub card_status: UimPhysicalCardState,
    /// Current state of the physical slot.
    pub slot_status: UimSlotState,
    /// Logical slot associated with this physical slot.
    ///
    /// This value is meaningful when the physical slot is active.
    pub logical_slot: u8,
    /// ICCID encoded as raw bytes, when available.
    pub iccid: Vec<u8>,
    /// Card protocol reported for the slot, when extended slot information
    /// is available.
    pub card_protocol: Option<UimCardProtocol>,
    /// Number of valid applications reported for the card.
    pub valid_applications: Option<u8>,
    /// Whether the slot contains an embedded UICC (eUICC).
    pub is_euicc: Option<bool>,
    /// Embedded UICC identifier, when available.
    ///
    /// The value is kept in its raw encoded representation.
    pub eid: Option<Vec<u8>>,
}

/// Status information for the physical UIM slots reported by the device.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, Default)]
pub struct UimSlotStatus {
    /// Physical UIM slots reported by the device.
    pub slots: Vec<UimPhysicalSlotStatus>,
}

#[cfg(feature = "v1_32")]
unsafe fn get_slot_status_extractor(
    output: *mut QmiMessageUimGetSlotStatusOutput,
    error: *mut *mut GError,
    param: &mut UimSlotStatus,
) -> gboolean {
    let mut physical_slots: *mut GArray = null_mut();

    let ret = qmi_message_uim_get_slot_status_output_get_physical_slot_status(
        output,
        &mut physical_slots,
        error,
    );

    if ret == GFALSE || physical_slots.is_null() {
        return ret;
    }

    let mut physical_information: *mut GArray = null_mut();
    let mut slot_eid: *mut GArray = null_mut();

    // Optional TLV: failure simply means that extended information isn't
    // available for this device.
    qmi_message_uim_get_slot_status_output_get_physical_slot_information(
        output,
        &mut physical_information,
        null_mut(),
    );

    let ret_eid =
        qmi_message_uim_get_slot_status_output_get_slot_eid(output, &mut slot_eid, null_mut());

    let eid_elements = if ret_eid == GTRUE && !slot_eid.is_null() {
        Some(std::slice::from_raw_parts(
            (*slot_eid).data.cast::<QmiSlotEidElement>(),
            (*slot_eid).len as usize,
        ))
    } else {
        None
    };

    let slots = std::slice::from_raw_parts(
        (*physical_slots).data.cast::<QmiPhysicalSlotStatusSlot>(),
        (*physical_slots).len as usize,
    );

    let information = if !physical_information.is_null()
        && (*physical_information).len == (*physical_slots).len
    {
        Some(std::slice::from_raw_parts(
            (*physical_information)
                .data
                .cast::<QmiPhysicalSlotInformationSlot>(),
            (*physical_information).len as usize,
        ))
    } else {
        None
    };

    param.slots = slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let info = information.map(|values| &values[index]);

            let iccid = if slot.iccid.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(
                    (*slot.iccid).data.cast::<u8>(),
                    (*slot.iccid).len as usize,
                )
                .to_vec()
            };
            let eid = eid_elements
                .and_then(|elements| elements.get(index))
                .and_then(|element| {
                    if element.eid.is_null() || (*element.eid).len == 0 {
                        None
                    } else {
                        Some(
                            std::slice::from_raw_parts(
                                (*element.eid).data.cast::<u8>(),
                                (*element.eid).len as usize,
                            )
                            .to_vec(),
                        )
                    }
                });

            UimPhysicalSlotStatus {
                card_status: from_glib(slot.physical_card_status),
                slot_status: from_glib(slot.physical_slot_status),
                logical_slot: slot.logical_slot,
                iccid,
                card_protocol: info.map(|value| from_glib(value.card_protocol)),
                valid_applications: info.map(|value| value.valid_applications),
                is_euicc: info.map(|value| from_glib(value.is_euicc)),
                eid,
            }
        })
        .collect();

    GTRUE
}

getter_with_extractor!(
    /// Retrieves the status of the physical UIM slots reported by the device.
    ///
    /// The returned information describes the available physical slots and
    /// their current state and configuration.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    get_slot_status,
    get_slot_status_extractor,
    UimSlotStatus,
    qmi_client_uim_get_slot_status,
    qmi_client_uim_get_slot_status_finish,
    qmi_message_uim_get_slot_status_output_get_result,
    qmi_message_uim_get_slot_status_output_unref,
    "get_slot_status"
);

#[cfg(feature = "v1_18")]
unsafe fn power_off_sim_writer(
    input: *mut QmiMessageUimPowerOffSimInput,
    error: *mut *mut GError,
    slot: u8,
) -> gboolean {
    qmi_message_uim_power_off_sim_input_set_slot(input, slot, error)
}

setter_with_writer!(
    /// Powers off the SIM in the specified physical slot.
    ///
    /// The supplied slot identifies the SIM whose power should be disabled.
    #[cfg(feature = "v1_18")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_18")))]
    power_off_sim,
    power_off_sim_writer,
    slot: u8,
    qmi_message_uim_power_off_sim_input_new,
    qmi_message_uim_power_off_sim_input_unref,
    qmi_client_uim_power_off_sim,
    qmi_client_uim_power_off_sim_finish,
    qmi_message_uim_power_off_sim_output_get_result,
    qmi_message_uim_power_off_sim_output_unref,
    "power_off_sim"
);

#[cfg(feature = "v1_18")]
unsafe fn power_on_sim_writer(
    input: *mut QmiMessageUimPowerOnSimInput,
    error: *mut *mut GError,
    slot: u8,
) -> gboolean {
    qmi_message_uim_power_on_sim_input_set_slot(input, slot, error)
}

setter_with_writer!(
    /// Powers on the SIM in the specified physical slot.
    ///
    /// The supplied slot identifies the SIM whose power should be enabled.
    #[cfg(feature = "v1_18")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_18")))]
    power_on_sim,
    power_on_sim_writer,
    slot: u8,
    qmi_message_uim_power_on_sim_input_new,
    qmi_message_uim_power_on_sim_input_unref,
    qmi_client_uim_power_on_sim,
    qmi_client_uim_power_on_sim_finish,
    qmi_message_uim_power_on_sim_output_get_result,
    qmi_message_uim_power_on_sim_output_unref,
    "power_on_sim"
);

#[cfg(feature = "v1_26")]
#[derive(Debug, Clone, Copy)]
pub struct UimSlotMapping {
    pub logical_slot: u8,
    pub physical_slot: u8,
}

#[cfg(feature = "v1_26")]
unsafe fn switch_slot_writer(
    input: *mut QmiMessageUimSwitchSlotInput,
    error: *mut *mut GError,
    mapping: UimSlotMapping,
) -> gboolean {
    let ret =
        qmi_message_uim_switch_slot_input_set_logical_slot(input, mapping.logical_slot, error);

    if ret == GFALSE {
        return GFALSE;
    }

    qmi_message_uim_switch_slot_input_set_physical_slot(input, mapping.physical_slot.into(), error)
}

setter_with_writer!(
    /// Changes the mapping between a logical UIM slot and a physical slot.
    ///
    /// The supplied mapping specifies the logical slot and the physical slot
    /// that should be associated with it.
    #[cfg(feature = "v1_26")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_26")))]
    switch_slot,
    switch_slot_writer,
    slot_mapping: UimSlotMapping,
    qmi_message_uim_switch_slot_input_new,
    qmi_message_uim_switch_slot_input_unref,
    qmi_client_uim_switch_slot,
    qmi_client_uim_switch_slot_finish,
    qmi_message_uim_switch_slot_output_get_result,
    qmi_message_uim_switch_slot_output_unref,
    "switch_slot"
);
