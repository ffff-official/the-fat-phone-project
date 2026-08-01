#[cfg(feature = "v1_26")]
mod all_call_status;
#[cfg(feature = "v1_26")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_26")))]
pub use all_call_status::VoiceCall;
#[cfg(feature = "v1_26")]
pub use libqmi_glib_types_rs::{
    IndicationVoiceAllCallStatusOutputCallInformationCall, VoicePresentation,
};

use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GFALSE, gboolean};
use gobject_sys::GObject;

use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::rc::Rc;

/// A client for interacting with the Voice service of a QMI device.
///
/// [`VoiceClient`] provides asynchronous operations for managing voice calls
/// through a QMI-compatible modem.
///
/// It can be used to:
///
/// - register for call status indications;
/// - initiate outgoing calls;
/// - answer incoming calls;
/// - terminate active calls.
///
/// Call state changes are reported asynchronously through the
/// `all-call-status` indication. Each call is identified by a modem-assigned
/// call identifier, which is required by operations such as answering or
/// ending a call.
///
/// The underlying QMI client is reference-counted and remains valid while at
/// least one [`VoiceClient`] instance exists.
#[derive(Debug)]
pub struct VoiceClient {
    client: NonNull<QmiClientVoice>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(VoiceClient);

client_clone_impl!(VoiceClient);

impl VoiceClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(
        VoiceClient,
        QmiClientVoice,
        libqmi_glib_types_rs::Service::Voice
    );

    close_client_impl!("voice");
}

allocate_client_callback!(allocate_client_cb, QmiClientVoice, "allocate VOICE client");

release_client_callback!(release_client_cb, "release VOICE client");

type Context = VoiceClient;

unsafe fn dial_call_writer(
    input: *mut QmiMessageVoiceDialCallInput,
    error: *mut *mut GError,
    number: impl AsRef<str>,
) -> gboolean {
    let number = CString::new(number.as_ref()).expect("Phone number contains a NULL byte");
    qmi_message_voice_dial_call_input_set_calling_number(input, number.as_ptr(), error)
}

unsafe fn dial_call_extractor(
    output: *mut QmiMessageVoiceDialCallOutput,
    error: *mut *mut GError,
) -> Result<u8, ()> {
    let mut call_id = 0;
    let ret = qmi_message_voice_dial_call_output_get_call_id(output, &mut call_id, error);
    match ret {
        GFALSE => Err(()),
        _ => Ok(call_id),
    }
}

request_with_writer_and_extractor!(
    /// Asynchronously requests the device to dial the specified phone number.
    ///
    /// Returns the call identifier assigned by the device.
    dial_call,
    dial_call_writer,
    phone_number: impl AsRef<str>,
    qmi_message_voice_dial_call_input_new,
    qmi_message_voice_dial_call_input_unref,
    qmi_client_voice_dial_call,
    qmi_client_voice_dial_call_finish,
    qmi_message_voice_dial_call_output_get_result,
    qmi_message_voice_dial_call_output_unref,
    u8,
    dial_call_extractor,
    "dial_call"
);

unsafe fn answer_call_writer(
    input: *mut QmiMessageVoiceAnswerCallInput,
    error: *mut *mut GError,
    id: u8,
) -> gboolean {
    qmi_message_voice_answer_call_input_set_call_id(input, id, error)
}

unsafe fn answer_call_extractor(
    output: *mut QmiMessageVoiceAnswerCallOutput,
    error: *mut *mut GError,
) -> Result<u8, ()> {
    let mut call_id = 0;
    let ret = qmi_message_voice_answer_call_output_get_call_id(output, &mut call_id, error);
    match ret {
        GFALSE => Err(()),
        _ => Ok(call_id),
    }
}

request_with_writer_and_extractor!(
    /// Asynchronously requests the device to answer to the specified call.
    ///
    /// Returns the call identifier reported by the device.
    answer_call,
    answer_call_writer,
    call_id: u8,
    qmi_message_voice_answer_call_input_new,
    qmi_message_voice_answer_call_input_unref,
    qmi_client_voice_answer_call,
    qmi_client_voice_answer_call_finish,
    qmi_message_voice_answer_call_output_get_result,
    qmi_message_voice_answer_call_output_unref,
    u8,
    answer_call_extractor,
    "answer_call"
);

unsafe fn end_call_writer(
    input: *mut QmiMessageVoiceEndCallInput,
    error: *mut *mut GError,
    id: u8,
) -> gboolean {
    qmi_message_voice_end_call_input_set_call_id(input, id, error)
}

unsafe fn end_call_extractor(
    output: *mut QmiMessageVoiceEndCallOutput,
    error: *mut *mut GError,
) -> Result<u8, ()> {
    let mut call_id = 0;
    let ret = qmi_message_voice_end_call_output_get_call_id(output, &mut call_id, error);
    match ret {
        GFALSE => Err(()),
        _ => Ok(call_id),
    }
}

request_with_writer_and_extractor!(
    /// Asynchronously requests the device to end the specified call.
    ///
    /// Returns the call identifier reported by the device.
    end_call,
    end_call_writer,
    call_id: u8,
    qmi_message_voice_end_call_input_new,
    qmi_message_voice_end_call_input_unref,
    qmi_client_voice_end_call,
    qmi_client_voice_end_call_finish,
    qmi_message_voice_end_call_output_get_result,
    qmi_message_voice_end_call_output_unref,
    u8,
    end_call_extractor,
    "end_call"
);

/// Input parameters for sending a burst of DTMF tones during a voice call.
///
/// A burst DTMF request sends one or more DTMF digits to an active call.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
pub struct VoiceBurstDtmfInput {
    /// Identifier of the call on which the DTMF tones should be sent.
    pub call_id: u8,
    /// Sequence of DTMF digits to send.
    pub digits: String,
}

#[cfg(feature = "v1_32")]
unsafe fn dtmf_tones_request_writer(
    input: *mut QmiMessageVoiceBurstDtmfInput,
    error: *mut *mut GError,
    dtmf: VoiceBurstDtmfInput,
) -> gboolean {
    let digits = CString::new(dtmf.digits).expect("Phone number contains a NULL byte");
    qmi_message_voice_burst_dtmf_input_set_data(input, dtmf.call_id, digits.as_ptr(), error)
}

#[cfg(feature = "v1_32")]
unsafe fn dtmf_tones_request_extractor(
    output: *mut QmiMessageVoiceBurstDtmfOutput,
    error: *mut *mut GError,
) -> Result<u8, ()> {
    let mut call_id = 0;
    let ret = qmi_message_voice_burst_dtmf_output_get_call_id(output, &mut call_id, error);
    match ret {
        GFALSE => Err(()),
        _ => Ok(call_id),
    }
}

request_with_writer_and_extractor!(
    /// Sends DTMF tones on an active voice call.
    ///
    /// Returns the call identifier reported by the modem.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    dtmf_tones_request,
    dtmf_tones_request_writer,
    input: VoiceBurstDtmfInput,
    qmi_message_voice_burst_dtmf_input_new,
    qmi_message_voice_burst_dtmf_input_unref,
    qmi_client_voice_burst_dtmf,
    qmi_client_voice_burst_dtmf_finish,
    qmi_message_voice_burst_dtmf_output_get_result,
    qmi_message_voice_burst_dtmf_output_unref,
    u8,
    dtmf_tones_request_extractor,
    "dtmf_tones_request"
);
