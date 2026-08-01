use super::*;

pub type EventReportCallback = Box<dyn FnMut(QmiResult<NasEventReport>)>;

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

unsafe fn set_event_report_writer(
    input: *mut QmiMessageNasSetEventReportInput,
    _error: *mut *mut GError,
    radio_interfaces: &[DmsRadioInterface],
) -> gboolean {
    qmi_message_nas_set_event_report_input_set_rssi_indicator(input, GTRUE, 1, null_mut());

    if radio_interfaces.contains(&DmsRadioInterface::Umts)
        || radio_interfaces.contains(&DmsRadioInterface::Cdma20001x)
    {
        qmi_message_nas_set_event_report_input_set_ecio_indicator(input, GTRUE, 1, null_mut());
    }

    if radio_interfaces.contains(&DmsRadioInterface::Cdma20001x) {
        qmi_message_nas_set_event_report_input_set_sinr_indicator(input, GTRUE, 1, null_mut());
    }

    if radio_interfaces.contains(&DmsRadioInterface::Lte) {
        qmi_message_nas_set_event_report_input_set_lte_rsrp_delta(input, GTRUE, 1, null_mut());
    }
    GTRUE
}

type Context = NasClient;

event!(
    /// Configures and enables NAS event report indications.
    ///
    /// The callback is invoked whenever the modem emits a NAS event report with
    /// signal information supported by the current radio technology.
    ///
    /// The [`DmsClient`] must be used to determine the radio interfaces supported by the
    /// device so that only compatible signal reports are requested. [`DmsClient::get_capabilities`]
    ///
    /// # Errors
    ///
    /// Returns an error if the event report configuration cannot be applied to the
    /// modem.
    set_event_report,
    EventReportCallback,
    dms_radio_interfaces: &[DmsRadioInterface],
    c"event-report",
    libqmi_glib_sys::QmiIndicationNasEventReportOutput,
    event_report_cb,
    set_event_report_writer,
    qmi_message_nas_set_event_report_input_new,
    qmi_message_nas_set_event_report_input_unref,
    qmi_client_nas_set_event_report,
    qmi_client_nas_set_event_report_finish,
    qmi_message_nas_set_event_report_output_get_result,
    qmi_message_nas_set_event_report_output_unref,
    "set_event_report"
);

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
