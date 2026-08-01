use super::*;

use {
    glib_sys::GArray,
    gobject_sys::GClosure,
    libqmi_glib_types_rs::glib::translate::{FromGlib, IntoGlib, from_glib_none},
};
type CallInformation = IndicationVoiceAllCallStatusOutputCallInformationCall;
type CallInfoCallback = Box<dyn FnMut(QmiResult<Vec<VoiceCall>>)>;

/// Describes a voice call reported by the modem.
///
/// This structure combines the call information reported in the
/// `all-call-status` indication with the remote party number, providing a
/// single, convenient view of the call.
#[cfg_attr(docsrs, doc(cfg(feature = "v1_26")))]
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
    /// Asynchronously sends a Set All Call Status request to the device.
    ///
    /// The sender parameter is the communication channel through which newly received new call states
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_26")))]
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
}

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
    let cb = &mut *(user_data.cast::<CallInfoCallback>());
    if ret == GFALSE || garray.is_null() {
        cb(Err(Into::<QmiError>::into((
            error,
            "call event informations",
        ))));
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
            cb(Err(Into::<QmiError>::into((
                error,
                "call event party number",
            ))));
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
            cb(Ok(calls));
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
