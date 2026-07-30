pub use libqmi_glib_types_rs::{NasEvdoSinrLevel, NasRadioInterface, NasRatModePreference};
pub use signal_info::SignalInfo;

pub mod signal_info;

use futures::channel::oneshot::{self, Sender};
use gio_sys::GAsyncResult;
use glib_sys::{GFALSE, GTRUE, gboolean};
use gobject_sys::{GClosure, GObject};
use libqmi_glib_types_rs::DmsRadioInterface;
use libqmi_glib_types_rs::glib::translate::from_glib;

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

type EventReportCallback = Box<dyn FnMut(QmiResult<NasEventReport>)>;

client_drop_impl!(NasClient);

client_clone_impl!(NasClient);

/// TODO : Documentation
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct NasEventReport {
    pub radio_interface: NasRadioInterface,

    // common
    pub rssi: Option<u8>,

    // 3G
    pub ecio: Option<u8>,

    // 3G China
    pub sinr: Option<NasEvdoSinrLevel>,

    // 4G LTE
    pub lte_rsrp: Option<i16>,
    // pub ss_rsrp: Option<i16>,
    // pub ss_rsrq: Option<i8>,
    // pub ss_sinr: Option<i8>,
}

impl Default for NasEventReport {
    fn default() -> Self {
        Self {
            radio_interface: NasRadioInterface::Unknown,
            rssi: None,
            ecio: None,
            sinr: None,
            lte_rsrp: None,
            // pub ss_rsrp: None,
            // pub ss_rsrq: None,
            // pub ss_sinr: None,
        }
    }
}

impl NasClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(NasClient, QmiClientNas, libqmi_glib_types_rs::Service::Nas);

    close_client_impl!("nas");

    pub async fn get_mode_preference(&self) -> QmiResult<NasRatModePreference> {
        let (tx, rx) = oneshot::channel::<QmiResult<NasRatModePreference>>();

        unsafe {
            qmi_client_nas_get_system_selection_preference(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_mode_preference_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
        }

        rx.await.map_err(|_| QmiError::cb_droped())?
    }

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

allocate_client_callback!(allocate_client_cb, QmiClientNas, "allocate NAS client");

release_client_callback!(release_client_cb, "release NAS client");

struct OutputGuard(*mut QmiMessageNasGetSystemSelectionPreferenceOutput);

impl Drop for OutputGuard {
    fn drop(&mut self) {
        unsafe {
            qmi_message_nas_get_system_selection_preference_output_unref(self.0);
        }
    }
}

unsafe extern "C" fn get_mode_preference_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender =
        Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<NasRatModePreference>>>());

    let result = get_mode_preference_result(client, res);

    drop(sender.send(result).inspect_err(cannot_send));
}

unsafe fn get_mode_preference_result(
    client: *mut GObject,
    res: *mut GAsyncResult,
) -> QmiResult<NasRatModePreference> {
    let mut error: *mut GError = null_mut();

    let output =
        qmi_client_nas_get_system_selection_preference_finish(client.cast(), res, &mut error);
    if output.is_null() {
        return Err((error, "get_mode_preference_cb finish").into());
    }

    let _guard = OutputGuard(output);

    let ret = qmi_message_nas_get_system_selection_preference_output_get_result(output, &mut error);
    if ret == GFALSE {
        return Err((error, "get_mode_preference_cb get result").into());
    }

    let mut mode_preference = 0;
    let ret = qmi_message_nas_get_system_selection_preference_output_get_mode_preference(
        output,
        &mut mode_preference,
        &mut error,
    );
    if ret == GFALSE {
        return Err((error, "get_mode_preference_cb output").into());
    }
    Ok(from_glib(mode_preference))
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
