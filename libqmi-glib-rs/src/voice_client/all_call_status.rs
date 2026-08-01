use super::*;

pub type CallInformation = IndicationVoiceAllCallStatusOutputCallInformationCall;
pub type CallInfoCallback = Box<dyn FnMut(QmiResult<Vec<VoiceCall>>)>;

use glib_sys::GArray;
use libqmi_glib_types_rs::glib::translate::{FromGlib, IntoGlib, from_glib_none};

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

unsafe fn set_all_call_status_writer(
    input: *mut QmiMessageVoiceIndicationRegisterInput,
    error: *mut *mut GError,
    _unused: (),
) -> gboolean {
    qmi_message_voice_indication_register_input_set_call_notification_events(
        input,
        true.into_glib(),
        error,
    )
}

event!(
    /// Enables voice call status indications.
    ///
    /// The supplied callback is invoked whenever the modem reports a
    /// change in the state of one or more voice calls.
    ///
    /// Call status indications include events such as incoming calls,
    /// call establishment, call termination, and state transitions.
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_26")))]
    set_all_call_status,
    CallInfoCallback,
    unused: (),
    c"all-call-status",
    libqmi_glib_sys::QmiIndicationVoiceAllCallStatusOutput,
    all_call_status_cb,
    set_all_call_status_writer,
    qmi_message_voice_indication_register_input_new,
    qmi_message_voice_indication_register_input_unref,
    qmi_client_voice_indication_register,
    qmi_client_voice_indication_register_finish,
    qmi_message_voice_indication_register_output_get_result,
    qmi_message_voice_indication_register_output_unref,
    "set_all_call_status"
);

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
