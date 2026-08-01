use super::*;

pub type EventReportCallback = Box<dyn FnMut(QmiResult<(&[u8], WmsMessageFormat)>)>;

unsafe fn set_event_report_writer(
    input: *mut QmiMessageWmsSetEventReportInput,
    error: *mut *mut GError,
    _unused: (),
) -> gboolean {
    qmi_message_wms_set_event_report_input_set_new_mt_message_indicator(input, 1, error)
}

event!(
    /// Asynchronously sends a Set Event Report request to the device.
    /// The sender parameter is the communication channel through which newly received SMS PDUs are sent.
    set_event_report,
    EventReportCallback,
    unused: (),
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
    "set_event_report"
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
