use super::*;

use gobject_sys::GClosure;

type EventReportCallback = Box<dyn FnMut(QmiResult<NasEventReport>)>;

/// Signal measurements reported by a NAS event report.
///
/// The available measurements depend on the radio interface and on the
/// information provided by the modem for the current radio technology.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct NasEventReport {
    /// Radio interface associated with the reported signal measurements.
    pub radio_interface: NasRadioInterface,
    /// Received Signal Strength Indicator (RSSI), when available.
    pub rssi: Option<u8>,
    /// Ec/Io signal quality measurement, when available.
    pub ecio: Option<u8>,
    /// EV-DO signal-to-interference-plus-noise ratio level, when available.
    pub sinr: Option<NasEvdoSinrLevel>,
    /// LTE Reference Signal Received Power (RSRP), when available.
    pub lte_rsrp: Option<i16>,
}

impl Default for NasEventReport {
    fn default() -> Self {
        Self {
            radio_interface: NasRadioInterface::Unknown,
            rssi: None,
            ecio: None,
            sinr: None,
            lte_rsrp: None,
        }
    }
}

impl NasClient {
    /// Configures and enables NAS event report indications.
    ///
    /// The callback is invoked whenever the modem emits a NAS event report with
    /// signal information supported by the current radio technology.
    ///
    /// The DMS client is used to determine the radio interfaces supported by the
    /// device so that only compatible signal reports are requested.
    ///
    /// # Errors
    ///
    /// Returns an error if the event report configuration cannot be applied to the
    /// modem.
    pub async fn set_event_report(
        &mut self,
        dms_client: &DmsClient,
        cb: EventReportCallback,
    ) -> QmiResult<()> {
        unsafe {
            let cb_ptr = Box::into_raw(Box::new(cb)).cast();
            let handler_id = gobject_sys::g_signal_connect_data(
                self.client.as_ptr().cast(),
                c"event-report".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *mut gobject_sys::GObject,
                        *mut libqmi_glib_sys::QmiIndicationNasEventReportOutput,
                        *mut std::ffi::c_void,
                    ),
                    unsafe extern "C" fn(),
                >(event_report_cb)),
                cb_ptr,
                Some(destroy_event_report_data),
                0,
            );
            if handler_id == 0 {
                drop(Box::from_raw(cb_ptr.cast::<EventReportCallback>()));
                Err(QmiError::new("handler id = 0", "set_event_report"))?;
            } else {
                tracing::debug!("Receiving evt report handler id = {}", handler_id);
            }
            let input = qmi_message_nas_set_event_report_input_new();

            let radio_interfaces = dms_client.get_capabilities().await?.radio_interfaces;

            #[rustfmt::skip]
            let set_indicator = |f: unsafe extern "C" fn(
                *mut QmiMessageNasSetEventReportInput, gboolean, u8, *mut *mut GError) -> gboolean,
                delta: u8|
             -> QmiResult<()> {
                let mut error: *mut GError = null_mut();
                let ret = f(input, GTRUE, delta, &mut error);
                if ret == GFALSE {
                    gobject_sys::g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                    qmi_message_nas_set_event_report_input_unref(input);
                    Err((error, "set_event_report set indicator"))?;
                }
                Ok(())
            };

            set_indicator(qmi_message_nas_set_event_report_input_set_rssi_indicator, 1)?;

            if radio_interfaces.contains(&DmsRadioInterface::Umts)
                || radio_interfaces.contains(&DmsRadioInterface::Cdma20001x)
            {
                set_indicator(qmi_message_nas_set_event_report_input_set_ecio_indicator, 1)?;
            }

            if radio_interfaces.contains(&DmsRadioInterface::Cdma20001x) {
                set_indicator(qmi_message_nas_set_event_report_input_set_sinr_indicator, 1)?;
            }

            if radio_interfaces.contains(&DmsRadioInterface::Lte) {
                set_indicator(qmi_message_nas_set_event_report_input_set_lte_rsrp_delta, 1)?;
            }

            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            qmi_client_nas_set_event_report(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(set_event_report_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );

            qmi_message_nas_set_event_report_input_unref(input);

            let ret = rx.await.map_err(|_| QmiError::cb_droped())?;
            tracing::debug!("NAS ready, waiting for messages...");
            ret
        }
    }
}

unsafe extern "C" fn set_event_report_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_nas_set_event_report_finish(client.cast(), res, &mut error);
    if output.is_null() {
        let _drop: Result<(), Result<(), QmiError>> = sender
            .send(Err((error, "nas event report").into()))
            .inspect_err(cannot_send);
        return;
    }

    let ret = qmi_message_nas_set_event_report_output_get_result(output, &mut error);
    qmi_message_nas_set_event_report_output_unref(output);

    let output: Result<(), _> = match ret {
        glib_sys::GFALSE => Err((error, "nas event report").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn event_report_cb(
    _client: *mut GObject,
    output: *mut QmiIndicationNasEventReportOutput,
    user_data: *mut c_void,
) {
    tracing::debug!("NAS event-report received");

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let mut radio_interface = QMI_NAS_RADIO_INTERFACE_UNKNOWN;
    let ret_rssi = qmi_indication_nas_event_report_output_get_rssi(
        output,
        &mut rssi,
        &mut radio_interface,
        &mut error,
    );
    if !error.is_null() {
        g_clear_error(&mut error);
    }

    let mut error: *mut GError = null_mut();
    let mut ecio = 0;
    let ret_ecio =
        qmi_indication_nas_event_report_output_get_ecio(output, &mut ecio, null_mut(), &mut error);
    if !error.is_null() {
        g_clear_error(&mut error);
    }

    let mut error: *mut GError = null_mut();
    let mut sinr: i32 = QMI_NAS_EVDO_SINR_LEVEL_0;
    let ret_sinr = qmi_indication_nas_event_report_output_get_sinr(output, &mut sinr, &mut error);
    if !error.is_null() {
        g_clear_error(&mut error);
    }

    let mut error: *mut GError = null_mut();
    let mut lte_rsrp: i16 = 0;
    let ret_lte_rsrp =
        qmi_indication_nas_event_report_output_get_lte_rsrp(output, &mut lte_rsrp, &mut error);
    if !error.is_null() {
        g_clear_error(&mut error);
    }

    fn ret_or<T>(ret: i32, value: T) -> Option<T> {
        if ret == GFALSE { None } else { Some(value) }
    }
    let response = NasEventReport {
        radio_interface: from_glib(radio_interface),
        rssi: ret_or(ret_rssi, rssi),
        ecio: ret_or(ret_ecio, ecio),
        sinr: ret_or(ret_sinr, from_glib(sinr)),
        lte_rsrp: ret_or(ret_lte_rsrp, lte_rsrp),
    };

    let cb = &mut *(user_data.cast::<EventReportCallback>());
    cb(Ok(response));
}

// Lauched when no more instances of GObject references QmiClientNas exists
unsafe extern "C" fn destroy_event_report_data(data: *mut c_void, _closure: *mut GClosure) {
    tracing::debug!("destroy_event_report_data");
    drop(Box::from_raw(data.cast::<EventReportCallback>()));
}
