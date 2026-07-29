use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GFALSE, GTRUE, g_array_unref};
use gobject_sys::{GClosure, GObject};
use libqmi_glib_types_rs::slice_to_garray;

use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

/// A client for interacting with the Network Access Service (NAS) of a QMI device.
#[derive(Debug)]
pub struct NasClient {
    client: NonNull<QmiClientNas>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

type EventReportCallback = Box<dyn FnMut(QmiResult<i8>)>;

client_drop_impl!(NasClient);

client_clone_impl!(NasClient);

impl NasClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(NasClient, QmiClientNas, libqmi_glib_types_rs::Service::Nas);

    close_client_impl!("nas");

    pub async fn set_event_report(&mut self, cb: EventReportCallback) -> QmiResult<()> {
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

            let mut error: *mut GError = null_mut();
            let thresholds: &[i8] = &[-110, -100, -90, -80, -70];
            let garray = slice_to_garray(thresholds);
            let ret = qmi_message_nas_set_event_report_input_set_signal_strength_indicator(
                input, GTRUE, garray, &mut error,
            );
            if ret == 0 {
                qmi_message_nas_set_event_report_input_unref(input);
                g_array_unref(garray);
                gobject_sys::g_signal_handler_disconnect(self.client.as_ptr().cast(), handler_id);
                Err((error, "set_event_report"))?;
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
            g_array_unref(garray);

            let ret = rx.await.map_err(|_| QmiError::cb_droped())?;
            tracing::debug!("NAS ready, waiting for messages...");
            ret
        }
    }
}

allocate_client_callback!(allocate_client_cb, QmiClientNas, "allocate NAS client");

release_client_callback!(release_client_cb, "release NAS client");

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

    let mut signal_strength: i8 = 0;
    let ret = qmi_indication_nas_event_report_output_get_signal_strength(
        output,
        &mut signal_strength,
        null_mut(),
        &mut error,
    );
    let cb = &mut *(user_data.cast::<EventReportCallback>());
    if ret == GFALSE {
        cb(Err(Into::<QmiError>::into((error, "NAS event-report"))))
    } else {
        cb(Ok(signal_strength));
    }
}

// Lauched when no more instances of GObject references QmiClientNas exists
unsafe extern "C" fn destroy_event_report_data(data: *mut c_void, _closure: *mut GClosure) {
    tracing::debug!("destroy_event_report_data");
    drop(Box::from_raw(data.cast::<EventReportCallback>()));
}
