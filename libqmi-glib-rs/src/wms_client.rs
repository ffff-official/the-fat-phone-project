use super::*;

use crate::sys::*;
use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{
    GArray, GError, GFALSE, g_array_append_vals, g_array_sized_new, g_array_unref, guint16, guint32,
};
use gobject_sys::{GClosure, GObject};
use libqmi_glib_types_rs::MessageWmsGetRoutesOutputRouteListElement;
use libqmi_glib_types_rs::glib::translate::FromGlib;
use libqmi_glib_types_rs::glib::translate::IntoGlib;
use libqmi_glib_types_rs::slice_to_garray;
use libqmi_glib_types_rs::{WmsGsmUmtsRpCause, WmsGsmUmtsTpCause, WmsMessageFormat};

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

type Route = MessageWmsGetRoutesOutputRouteListElement;
type QmiRoute = QmiMessageWmsGetRoutesOutputRouteListElement;
type EventReportCallback = Box<dyn FnMut(QmiResult<(&[u8], WmsMessageFormat)>)>;

/// A client for interacting with the Wireless Messaging Service (WMS) of a QMI device.
///
/// [`WmsClient`] provides asynchronous operations for managing SMS messaging
/// through a QMI-compatible modem.
///
/// It can be used to:
///
/// - send SMS messages;
/// - configure SMS routing and event reporting;
/// - register for SMS and delivery report indications;
///
/// Incoming SMS messages, delivery reports, and other messaging events are
/// reported asynchronously through WMS indications.
///
/// The underlying QMI client is reference-counted and remains valid while at
/// least one [`WmsClient`] instance exists.
#[derive(Debug)]
pub struct WmsClient {
    client: NonNull<QmiClientWms>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(WmsClient);

client_clone_impl!(WmsClient);

impl WmsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(WmsClient, QmiClientWms, libqmi_glib_types_rs::Service::Wms);

    close_client_impl!("wms");

    /// Asynchronously sends a Set Event Report request to the device.
    /// The sender parameter is the communication channel through which newly received SMS PDUs are sent.
    pub async fn set_event_report(&mut self, cb: EventReportCallback) -> QmiResult<()> {
        unsafe {
            let cb_ptr = Box::into_raw(Box::new(cb)).cast();
            let handler_id = gobject_sys::g_signal_connect_data(
                self.client.as_ptr().cast(),
                c"event-report".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *mut gobject_sys::GObject,
                        *mut libqmi_glib_sys::QmiIndicationWmsEventReportOutput,
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

            let input = qmi_message_wms_set_event_report_input_new();

            let mut error: *mut GError = null_mut();
            let ret = qmi_message_wms_set_event_report_input_set_new_mt_message_indicator(
                input, 1, &mut error,
            );
            if ret == 0 {
                qmi_message_wms_set_event_report_input_unref(input);
                gobject_sys::g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                Err((error, "set_event_report"))?;
            }

            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            qmi_client_wms_set_event_report(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(set_event_report_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_wms_set_event_report_input_unref(input);

            let ret = rx.await.map_err(|_| QmiError::cb_droped())?;
            tracing::debug!("WMS ready, waiting for messages...");
            ret
        }
    }

    /// Asynchronously sends a Get Routes request to the device.
    pub async fn get_routes(&self) -> QmiResult<Vec<Route>> {
        let (tx, rx) = oneshot::channel::<QmiResult<Vec<Route>>>();

        unsafe {
            qmi_client_wms_get_routes(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_routes_cb),
                Box::into_raw(Box::new(tx)).cast(),
            )
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously sends a Set Routes request to the device.
    pub async fn set_routes(&mut self, routes: Vec<Route>) -> QmiResult<()> {
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

        let input: *mut QmiMessageWmsSetRoutesInput =
            unsafe { qmi_message_wms_set_routes_input_new() };
        let mut error: *mut GError = null_mut();

        let routes = routes
            .into_iter()
            .map(|route| route.into())
            .collect::<Vec<QmiMessageWmsGetRoutesOutputRouteListElement>>();

        let value_route_list = unsafe { slice_to_garray(routes.as_slice()) };

        let ret = unsafe {
            qmi_message_wms_set_routes_input_set_route_list(input, value_route_list, &mut error)
        };
        if ret == 0 {
            unsafe {
                qmi_message_wms_set_routes_input_unref(input);
                g_array_unref(value_route_list);
            }
            Err((error, "set routes"))?;
        }

        unsafe {
            qmi_client_wms_set_routes(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(set_routes_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
            qmi_message_wms_set_routes_input_unref(input);
            g_array_unref(value_route_list);
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Sends SMS by the specified TPDU to the network operator's SMSC.
    pub async fn send_sms(&mut self, sms_tpdu: Vec<u8>) -> QmiResult<QmiResult<u64>> {
        let (tx, rx) = oneshot::channel::<QmiResult<u64>>();

        let tpdu = unsafe { g_array_sized_new(GFALSE, GFALSE, 1, sms_tpdu.len() as _) };
        unsafe { g_array_append_vals(tpdu, sms_tpdu.as_ptr().cast(), sms_tpdu.len() as _) };

        unsafe {
            let len = (*tpdu).len as usize;
            let data = (*tpdu).data.cast::<u8>();

            if data.is_null() && len != 0 {
                tracing::debug!("GArray data is NULL but length is {len}");
                return Err(QmiError::new("Unexpected GLIB GArray", "send sms"));
            }

            let bytes = std::slice::from_raw_parts(data, len);

            tracing::debug!("GArray length: {}", bytes.len());
            drop(crate::tracing_debug_hex(bytes));
        }

        let mut error: *mut GError = null_mut();

        let input = unsafe { qmi_message_wms_raw_send_input_new() };
        let ret = unsafe {
            qmi_message_wms_raw_send_input_set_raw_message_data(
                input,
                WmsMessageFormat::GsmWcdmaPointToPoint.into_glib(),
                tpdu,
                &mut error,
            )
        };
        if ret == GFALSE {
            unsafe {
                g_array_unref(tpdu);
                qmi_message_wms_raw_send_input_unref(input);
            }
            Err((error, "send_sms"))?;
        }

        unsafe {
            qmi_client_wms_raw_send(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(send_sms_cb),
                Box::into_raw(Box::new(tx)).cast(),
            )
        }
        let ret: Result<u64, QmiError> = rx.await.map_err(|_| QmiError::cb_droped())?;
        unsafe {
            g_array_unref(tpdu);
            qmi_message_wms_raw_send_input_unref(input);
        }
        Ok(ret)
    }
}

allocate_client_callback!(allocate_client_cb, QmiClientWms, "allocate WMS client");

release_client_callback!(release_client_cb, "release WMS client");

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

// Lauched when no more instances of GObject references QmiClientWms exists
unsafe extern "C" fn destroy_event_report_data(data: *mut c_void, _closure: *mut GClosure) {
    tracing::debug!("destroy_event_report_data");
    drop(Box::from_raw(data.cast::<EventReportCallback>()));
}

unsafe extern "C" fn set_event_report_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_wms_set_event_report_finish(client.cast(), res, &mut error);
    if output.is_null() {
        let _drop = sender
            .send(Err((error, "wms event report").into()))
            .inspect_err(cannot_send);
        return;
    }

    let ret = qmi_message_wms_set_event_report_output_get_result(output, &mut error);
    qmi_message_wms_set_event_report_output_unref(output);

    let output: Result<(), _> = match ret {
        GFALSE => Err((error, "wms event report").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn get_routes_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<Vec<Route>>>>());
    let mut error: *mut GError = null_mut();

    let get_routes_output: *mut QmiMessageWmsGetRoutesOutput =
        qmi_client_wms_get_routes_finish(client.cast(), res, &mut error);
    if get_routes_output.is_null() {
        let _drop = sender
            .send(Err((error, "get routes").into()))
            .inspect_err(cannot_send);
        return;
    }

    let mut value_route_list: *mut GArray = null_mut();
    let ret = qmi_message_wms_get_routes_output_get_route_list(
        get_routes_output,
        &mut value_route_list,
        &mut error,
    );
    let output = match ret {
        GFALSE => Err((error, "wms event report").into()),
        _ => Ok(std::slice::from_raw_parts(
            (*value_route_list).data.cast::<QmiRoute>(),
            (*value_route_list).len as usize,
        )
        .iter()
        .map(|route| (*route).into())
        .collect::<Vec<Route>>()),
    };
    qmi_message_wms_get_routes_output_unref(get_routes_output);
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn set_routes_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

    let mut error: *mut GError = null_mut();

    let ret = qmi_client_wms_set_routes_finish(client.cast(), res, &mut error);

    let output = match ret.is_null() {
        true => Err((error, "set routes").into()),
        _ => Ok(()),
    };
    qmi_message_wms_set_routes_output_unref(ret);
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn send_sms_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<u64>>>());

    let mut error: *mut GError = null_mut();

    let raw = qmi_client_wms_raw_send_finish(client.cast(), res, &mut error);
    let output = if raw.is_null() {
        Err((error, "send_sms_cb 1").into())
    } else {
        let ret = qmi_message_wms_raw_send_output_get_result(raw, &mut error);
        let ret = match ret {
            GFALSE => {
                let mut rp_cause = 0;
                let mut tp_cause = 0;

                let has_cause = qmi_message_wms_raw_send_output_get_gsm_wcdma_cause_info(
                    raw,
                    &mut rp_cause,
                    &mut tp_cause,
                    null_mut(),
                );
                if has_cause != 0 {
                    tracing::debug!("{:?}", WmsGsmUmtsRpCause::from_glib(rp_cause));
                    tracing::debug!("{:?}", WmsGsmUmtsTpCause::from_glib(tp_cause));
                    tracing::debug!("SMS sending failed with GSM/UMTS cause");
                }
                Err((error, "send_sms_cb 2").into())
            }
            _ => {
                let mut message_id: guint16 = 0;
                let ret = qmi_message_wms_raw_send_output_get_message_id(
                    raw,
                    &mut message_id,
                    &mut error,
                );
                match ret {
                    GFALSE => Err((error, "send_sms_cb 3").into()),
                    _ => Ok(message_id as u64),
                }
            }
        };
        qmi_message_wms_raw_send_output_unref(raw);
        ret
    };
    drop(sender.send(output).inspect_err(cannot_send));
}
