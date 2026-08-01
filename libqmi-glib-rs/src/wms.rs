mod event_report;
pub use event_report::*;
pub type Route = MessageWmsGetRoutesOutputRouteListElement;

use glib_sys::GTRUE;
use glib_sys::gboolean;

use super::*;

use crate::sys::*;
use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{
    GArray, GError, GFALSE, g_array_append_vals, g_array_sized_new, g_array_unref, guint16, guint32,
};
use gobject_sys::GObject;
use libqmi_glib_types_rs::MessageWmsGetRoutesOutputRouteListElement;
use libqmi_glib_types_rs::glib::translate::FromGlib;
use libqmi_glib_types_rs::glib::translate::IntoGlib;
use libqmi_glib_types_rs::slice_to_garray;
use libqmi_glib_types_rs::{WmsGsmUmtsRpCause, WmsGsmUmtsTpCause, WmsMessageFormat};

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

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

    // TODO : This is bullshit.
    // This request does not fit request_with_writer_and_extractor! because
    // GSM/UMTS RP and TP failure causes must be extracted from the output
    // when get_result() fails.
    //
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

unsafe extern "C" fn send_sms_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<u64>>>());

    let mut error: *mut GError = null_mut();

    let raw = qmi_client_wms_raw_send_finish(client.cast(), res, &mut error);
    let output = if raw.is_null() {
        Err((error, "send_sms_cb finish").into())
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
                Err((error, "send_sms_cb get result").into())
            }
            _ => {
                let mut message_id: guint16 = 0;
                let ret = qmi_message_wms_raw_send_output_get_message_id(
                    raw,
                    &mut message_id,
                    &mut error,
                );
                match ret {
                    GFALSE => Err((error, "send_sms_cb get message id").into()),
                    _ => Ok(message_id as u64),
                }
            }
        };
        qmi_message_wms_raw_send_output_unref(raw);
        ret
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

type Context = WmsClient;

unsafe fn set_routes_writer(
    input: *mut QmiMessageWmsSetRoutesInput,
    error: *mut *mut GError,
    routes: Vec<Route>,
) -> gboolean {
    let routes = routes
        .into_iter()
        .map(|route| route.into())
        .collect::<Vec<QmiMessageWmsGetRoutesOutputRouteListElement>>();

    let value_route_list = unsafe { slice_to_garray(routes.as_slice()) };

    let ret = qmi_message_wms_set_routes_input_set_route_list(input, value_route_list, error);
    g_array_unref(value_route_list);
    ret
}

setter_with_writer!(
    /// Configures the WMS message routing rules used by the modem.
    ///
    /// Each route specifies how a particular message type and class should be
    /// handled, including the destination storage and receipt action.
    ///
    /// Replacing the routing configuration affects how subsequent incoming
    /// messages are delivered by the modem.
    set_routes,
    set_routes_writer,
    routes: Vec<Route>,
    qmi_message_wms_set_routes_input_new,
    qmi_message_wms_set_routes_input_unref,
    qmi_client_wms_set_routes,
    qmi_client_wms_set_routes_finish,
    qmi_message_wms_set_routes_output_get_result,
    qmi_message_wms_set_routes_output_unref,
    "set_routes"
);

unsafe fn get_routes_extractor(
    output: *mut QmiMessageWmsGetRoutesOutput,
    error: *mut *mut GError,
    param: &mut Vec<Route>,
) -> gboolean {
    let mut value_route_list: *mut GArray = null_mut();
    let ret =
        qmi_message_wms_get_routes_output_get_route_list(output, &mut value_route_list, error);
    if ret == GTRUE && !value_route_list.is_null() && (*value_route_list).len > 0 {
        *param = std::slice::from_raw_parts(
            (*value_route_list)
                .data
                .cast::<QmiMessageWmsGetRoutesOutputRouteListElement>(),
            (*value_route_list).len as usize,
        )
        .iter()
        .copied()
        .map(Route::from)
        .collect::<Vec<Route>>();
    }
    ret
}

getter_with_extractor!(
    /// Extracts the configured WMS message routes from a Get Routes response.
    ///
    /// The route list is owned by the QMI output message. Each element is therefore
    /// copied into an owned Rust [`Route`] before the output message is released.
    ///
    /// An empty route list produces an empty vector.
    get_routes,
    get_routes_extractor,
    Vec<Route>,
    qmi_client_wms_get_routes,
    qmi_client_wms_get_routes_finish,
    qmi_message_wms_get_routes_output_get_result,
    qmi_message_wms_get_routes_output_unref,
    "get_routes"
);
