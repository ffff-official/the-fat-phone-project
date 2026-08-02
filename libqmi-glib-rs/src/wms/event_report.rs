use super::*;

pub type EventReportCallback = Box<dyn FnMut(QmiResult<(&[u8], WmsMessageFormat)>)>;

unsafe fn set_event_report_writer(
    input: *mut QmiMessageWmsSetEventReportInput,
    error: *mut *mut GError,
    enabled: bool,
) -> gboolean {
    qmi_message_wms_set_event_report_input_set_new_mt_message_indicator(
        input,
        enabled.into_glib(),
        error,
    )
}

signal!(
    /// Registers a callback for incoming SMS event indications.
    ///
    /// The callback is invoked whenever the modem reports a newly received
    /// SMS transferred through the configured WMS route.
    ///
    /// The `enabled` parameter specifies whether the modem should immediately
    /// enable or disable SMS event indications. The signal handler remains
    /// connected regardless of this value, allowing indications to be
    /// temporarily suspended and later re-enabled without reconnecting the
    /// callback.
    signal_event_report,
    EventReportCallback,
    enabled: bool,
    c"event-report",
    libqmi_glib_sys::QmiIndicationWmsEventReportOutput,
    event_report_cb,
    set_event_report_writer,
    qmi_message_wms_set_event_report_input_new,
    qmi_message_wms_set_event_report_input_unref,
    qmi_client_wms_set_event_report,
    qmi_client_wms_set_event_report_finish,
    qmi_message_wms_set_event_report_output_get_result,
    qmi_message_wms_set_event_report_output_unref,
    "signal_event_report"
);

unsafe extern "C" fn event_report_cb(
    _client: *mut GObject,
    output: *mut QmiIndicationWmsEventReportOutput,
    user_data: *mut c_void,
) {
    tracing::debug!("WMS event-report received");

    let mut error: *mut GError = null_mut();

    let mut _value_transfer_route_mt_message_ack_indicator = 0;
    let mut _value_transfer_route_mt_message_transaction_id: guint32 = 0;
    let mut value_transfer_route_mt_message_format = 0;
    let mut value_transfer_route_mt_message_raw_data: *mut GArray = null_mut();

    let ret = qmi_indication_wms_event_report_output_get_transfer_route_mt_message(
        output,
        &mut _value_transfer_route_mt_message_ack_indicator,
        &mut _value_transfer_route_mt_message_transaction_id,
        &mut value_transfer_route_mt_message_format,
        &mut value_transfer_route_mt_message_raw_data,
        &mut error,
    );
    let cb = &mut *(user_data.cast::<EventReportCallback>());
    if ret == GFALSE {
        cb(Err(Into::<QmiError>::into((error, "message event TPDU"))));
    } else {
        let data = {
            let ptr = (*value_transfer_route_mt_message_raw_data).data.cast();
            let len = (*value_transfer_route_mt_message_raw_data).len as usize;
            std::slice::from_raw_parts(ptr, len)
        };
        let message_format = WmsMessageFormat::from_glib(value_transfer_route_mt_message_format);
        cb(Ok((data, message_format)));
    }
}

setter!(
    /// Enables or disables incoming SMS event indications.
    ///
    /// Disabling the indications does not disconnect an existing GLib
    /// signal handler. Re-enabling them allows the same handler to resume
    /// receiving event report updates.
    set_event_report_indications,
    qmi_message_wms_set_event_report_input_set_new_mt_message_indicator,
    enabled: bool,
    qmi_message_wms_set_event_report_input_new,
    qmi_message_wms_set_event_report_input_unref,
    qmi_client_wms_set_event_report,
    qmi_client_wms_set_event_report_finish,
    qmi_message_wms_set_event_report_output_get_result,
    qmi_message_wms_set_event_report_output_unref,
    "set_event_report_indications"
);
