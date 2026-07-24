use super::*;

use crate::sys::*;

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GArray, GFALSE};
use gobject_sys::{GClosure, GObject, g_object_ref, g_object_unref};
use libqmi_glib_types_rs::glib::translate::{FromGlib, IntoGlib, from_glib_none};
use libqmi_glib_types_rs::{
    DeviceReleaseClientFlags, IndicationVoiceAllCallStatusOutputCallInformationCall, Service,
    VoicePresentation,
};

use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::ptr::NonNull;
use std::ptr::null_mut;

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
}

impl Drop for VoiceClient {
    fn drop(&mut self) {
        tracing::trace!("VOICE CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr().cast());
        }
    }
}

impl Clone for VoiceClient {
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

type CallInformation = IndicationVoiceAllCallStatusOutputCallInformationCall;
type CallInfoCallback = Box<dyn FnMut(Vec<VoiceCall>)>;

/// Describes a voice call reported by the modem.
///
/// This structure combines the call information reported in the
/// `all-call-status` indication with the remote party number, providing a
/// single, convenient view of the call.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VoiceCall {
    /// General information about the call.
    pub info: CallInformation,
    /// Remote party phone number.
    pub number: String,
    /// Presentation indicator associated with the remote party number.
    ///
    /// This value indicates whether the number is allowed to be displayed,
    /// restricted, unavailable, or has another presentation status.
    pub presentation_indicator: VoicePresentation,
}

impl std::ops::Deref for VoiceCall {
    type Target = CallInformation;

    fn deref(&self) -> &Self::Target {
        &self.info
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

    /// Asynchronously releases the [`VoiceClient`] from the [`Device`].
    pub async fn close(self) -> QmiResult<()> {
        let ref_count = unsafe { *(self.client.as_ptr().cast::<GObject>()) }.ref_count;
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

    /// Asynchronously sends a Set All Call Status request to the device.
    ///
    /// The sender parameter is the communication channel through which newly received new call states
    pub async fn set_all_call_status(&mut self, cb: CallInfoCallback) -> QmiResult<()> {
        unsafe {
            let cb_ptr = Box::into_raw(Box::new(cb)).cast();
            let handler_id = gobject_sys::g_signal_connect_data(
                self.client.as_ptr().cast(),
                c"all-call-status".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *mut gobject_sys::GObject,
                        *mut libqmi_glib_sys::QmiIndicationVoiceAllCallStatusOutput,
                        *mut std::ffi::c_void,
                    ),
                    unsafe extern "C" fn(),
                >(all_call_status_cb)),
                cb_ptr,
                Some(destroy_all_call_status_data),
                0,
            );
            if handler_id == 0 {
                drop(Box::from_raw(cb_ptr.cast::<CallInfoCallback>()));
                Err(QmiError::new(
                    "handler id = 0",
                    "set_all_call_status g_signal",
                ))?;
            } else {
                tracing::debug!("Receiving all call status handler id = {}", handler_id);
            }

            let input = qmi_message_voice_indication_register_input_new();

            let mut error: *mut GError = null_mut();
            let ret = qmi_message_voice_indication_register_input_set_call_notification_events(
                input,
                true.into_glib(),
                &mut error,
            );
            if ret == 0 {
                qmi_message_voice_indication_register_input_unref(input);
                gobject_sys::g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                Err((error, "set_all_call_status set_call_notification_event"))?;
            }

            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            qmi_client_voice_indication_register(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(set_all_call_status_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_voice_indication_register_input_unref(input);

            let ret = rx.await.map_err(|_| QmiError::cb_droped())?;
            if ret.is_err() {
                gobject_sys::g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
            }
            tracing::debug!("VOICE ready, waiting for events...");
            ret
        }
    }

    /// Asynchronously requests the device to dial the specified phone number.
    ///
    /// Returns the call identifier assigned by the device.
    pub async fn dial_call(&self, number: impl AsRef<str>) -> QmiResult<u8> {
        let number = CString::new(number.as_ref())
            .map_err(|_e| QmiError::new("Phone number contains a NUL byte", "dial_call"))?;

        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let input = unsafe { qmi_message_voice_dial_call_input_new() };

        let mut error: *mut GError = null_mut();
        let ret = unsafe {
            qmi_message_voice_dial_call_input_set_calling_number(input, number.as_ptr(), &mut error)
        };
        if ret == GFALSE {
            unsafe { qmi_message_voice_dial_call_input_unref(input) }
            return Err((error, "dial_call set_calling_number").into());
        }
        unsafe {
            qmi_client_voice_dial_call(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(dial_call_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_voice_dial_call_input_unref(input);
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously requests the device to answer to the specified call.
    ///
    /// Returns the call identifier reported by the device.
    pub async fn answer_call(&self, id: u8) -> QmiResult<u8> {
        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let input = unsafe { qmi_message_voice_answer_call_input_new() };

        let mut error: *mut GError = null_mut();
        let ret = unsafe { qmi_message_voice_answer_call_input_set_call_id(input, id, &mut error) };
        if ret == GFALSE {
            unsafe { qmi_message_voice_answer_call_input_unref(input) }
            return Err((error, "answer_call set_call_id").into());
        }
        unsafe {
            qmi_client_voice_answer_call(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(answer_call_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_voice_answer_call_input_unref(input);
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously requests the device to end the specified call.
    ///
    /// Returns the call identifier reported by the device.
    pub async fn end_call(&self, id: u8) -> QmiResult<u8> {
        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let input = unsafe { qmi_message_voice_end_call_input_new() };

        let mut error: *mut GError = null_mut();
        let ret = unsafe { qmi_message_voice_end_call_input_set_call_id(input, id, &mut error) };
        if ret == GFALSE {
            unsafe { qmi_message_voice_end_call_input_unref(input) }
            return Err((error, "end_call set_call_id").into());
        }
        unsafe {
            qmi_client_voice_end_call(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(end_call_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_voice_end_call_input_unref(input);
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Sends DTMF tones on an active voice call.
    ///
    /// # Parameters
    ///
    /// - `id`: The identifier of the active call.
    /// - `digits`: The DTMF digits to send.
    ///
    /// # Returns
    ///
    /// Returns the call identifier reported by the modem.
    pub async fn dtmf_tones_request(&self, id: u8, digits: impl AsRef<str>) -> QmiResult<u8> {
        let (tx, rx) = oneshot::channel::<QmiResult<u8>>();

        let digits = CString::new(digits.as_ref())
            .map_err(|_e| QmiError::new("Trailing NULL value".to_string(), "dtmf_tones_request"))?;

        let input = unsafe { qmi_message_voice_burst_dtmf_input_new() };

        let mut error: *mut GError = null_mut();
        let ret = unsafe {
            qmi_message_voice_burst_dtmf_input_set_data(input, id, digits.as_ptr(), &mut error)
        };
        if ret == GFALSE {
            unsafe { qmi_message_voice_burst_dtmf_input_unref(input) }
            return Err((error, "end_call set_call_id").into());
        }
        unsafe {
            qmi_client_voice_burst_dtmf(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(dtmf_tones_request_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_voice_burst_dtmf_input_unref(input);
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }
}

allocate_client_callback!(allocate_client_cb, QmiClientVoice, "allocate VOICE client");

release_client_callback!(release_client_cb, "release VOICE client");

// IMPORTANT: Quick-and-dirty workaround to suppress duplicate incoming call events.
//            The modem occasionally reports the same incoming call twice.
thread_local! {
    static LAST_SNAPSHOTS: RefCell<Vec<VoiceCall>> =
        const { RefCell::new(Vec::new()) };
}

fn filter_duplicates(mut snapshots: Vec<VoiceCall>) -> Vec<VoiceCall> {
    LAST_SNAPSHOTS.with(|last_snapshots| {
        let mut last_snapshots = last_snapshots.borrow_mut();

        let current_snapshots = snapshots.clone();

        snapshots.retain(|snapshot| !last_snapshots.contains(snapshot));

        *last_snapshots = current_snapshots;

        snapshots
    })
}

unsafe extern "C" fn all_call_status_cb(
    _client: *mut GObject,
    output: *mut QmiIndicationVoiceAllCallStatusOutput,
    user_data: *mut c_void,
) {
    tracing::debug!("VOICE all call status received");

    let mut error: *mut GError = null_mut();

    let mut garray: *mut GArray = null_mut();
    let ret = qmi_indication_voice_all_call_status_output_get_call_information(
        output,
        &mut garray,
        &mut error,
    );
    if ret == GFALSE || garray.is_null() {
        tracing::error!(
            "{}",
            Into::<QmiError>::into((error, "call event informations"))
        );
    } else {
        let info_calls = {
            let ptr = (*garray)
                .data
                .cast::<QmiIndicationVoiceAllCallStatusOutputCallInformationCall>();
            let len = (*garray).len as usize;
            std::slice::from_raw_parts(ptr, len)
        };
        let mut garray: *mut GArray = null_mut();
        let ret = qmi_indication_voice_all_call_status_output_get_remote_party_number(
            output,
            &mut garray,
            &mut error,
        );
        if ret == GFALSE || garray.is_null() {
            tracing::error!(
                "{}",
                Into::<QmiError>::into((error, "call event party number"))
            );
        } else {
            let remote_party_numbers = {
                let ptr = (*garray)
                    .data
                    .cast::<QmiIndicationVoiceAllCallStatusOutputRemotePartyNumberCall>();
                let len = (*garray).len as usize;
                std::slice::from_raw_parts(ptr, len)
            };

            let calls = info_calls
                .iter()
                .map(|call| {
                    let party_number = remote_party_numbers
                        .iter()
                        .find(|remote| remote.id == call.id)
                        .map(|indic| {
                            (
                                from_glib_none(indic.type_.cast()),
                                indic.presentation_indicator,
                            )
                        });
                    VoiceCall {
                        info: CallInformation::from(*call),
                        presentation_indicator: party_number
                            .as_ref()
                            .map(|p| VoicePresentation::from_glib(p.1))
                            .unwrap_or(VoicePresentation::Unavailable),
                        number: party_number.map(|p| p.0).unwrap_or(String::new()),
                    }
                })
                .collect();
            let calls = filter_duplicates(calls);
            let cb = &mut *(user_data.cast::<CallInfoCallback>());
            cb(calls);
        }
    }
}

unsafe extern "C" fn set_all_call_status_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_voice_indication_register_finish(client.cast(), res, &mut error);
    if output.is_null() {
        let _drop = sender
            .send(Err((error, "set_all_call_status cb finish").into()))
            .inspect_err(cannot_send);
        return;
    }

    let ret = qmi_message_voice_indication_register_output_get_result(output, &mut error);
    qmi_message_voice_indication_register_output_unref(output);

    let output: Result<(), _> = match ret {
        GFALSE => Err((error, "set_all_call_status cb get_result").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

// Lauched when no more instances of GObject references QmiClientVoice exists
unsafe extern "C" fn destroy_all_call_status_data(data: *mut c_void, _closure: *mut GClosure) {
    tracing::debug!("destroy_all_call_status_data");
    drop(Box::from_raw(data.cast::<CallInfoCallback>()));
}

macro_rules! voice_call_callback {
    (
        $callback:ident,
        finish = $finish:path,
        get_result = $get_result:path,
        get_call_id = $get_call_id:path,
        unref = $unref:path,
        context = $context:literal
    ) => {
        unsafe extern "C" fn $callback(
            client: *mut GObject,
            res: *mut GAsyncResult,
            user_data: *mut c_void,
        ) {
            let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<u8>>>());

            let mut error: *mut GError = null_mut();

            let output_ptr = $finish(client.cast(), res, &mut error);

            if output_ptr.is_null() {
                drop(
                    sender
                        .send(Err((error, concat!($context, " finish")).into()))
                        .inspect_err(cannot_send),
                );
                return;
            }

            let output = if $get_result(output_ptr, &mut error) == GFALSE {
                Err((error, concat!($context, " get_result")).into())
            } else {
                let mut call_id = 0;

                if $get_call_id(output_ptr, &mut call_id, &mut error) == GFALSE {
                    Err((error, concat!($context, " get_call_id")).into())
                } else {
                    Ok(call_id)
                }
            };

            $unref(output_ptr);
            drop(sender.send(output).inspect_err(cannot_send));
        }
    };
}

voice_call_callback!(
    dial_call_cb,
    finish = qmi_client_voice_dial_call_finish,
    get_result = qmi_message_voice_dial_call_output_get_result,
    get_call_id = qmi_message_voice_dial_call_output_get_call_id,
    unref = qmi_message_voice_dial_call_output_unref,
    context = "dial_call cb"
);

voice_call_callback!(
    answer_call_cb,
    finish = qmi_client_voice_answer_call_finish,
    get_result = qmi_message_voice_answer_call_output_get_result,
    get_call_id = qmi_message_voice_answer_call_output_get_call_id,
    unref = qmi_message_voice_answer_call_output_unref,
    context = "answer_call cb"
);

voice_call_callback!(
    end_call_cb,
    finish = qmi_client_voice_end_call_finish,
    get_result = qmi_message_voice_end_call_output_get_result,
    get_call_id = qmi_message_voice_end_call_output_get_call_id,
    unref = qmi_message_voice_end_call_output_unref,
    context = "end_call cb"
);

voice_call_callback!(
    dtmf_tones_request_cb,
    finish = qmi_client_voice_burst_dtmf_finish,
    get_result = qmi_message_voice_burst_dtmf_output_get_result,
    get_call_id = qmi_message_voice_burst_dtmf_output_get_call_id,
    unref = qmi_message_voice_burst_dtmf_output_unref,
    context = "dtmf_tones_request cb"
);
