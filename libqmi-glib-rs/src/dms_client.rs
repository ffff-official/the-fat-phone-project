pub use libqmi_glib_types_rs::{DmsDataServiceCapability, DmsRadioInterface, DmsSimCapability};

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GArray, GFALSE};
use gobject_sys::GObject;
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

/// A client for interacting with the Device Management Service (DMS) of a QMI device.
#[derive(Debug)]
pub struct DmsClient {
    client: NonNull<QmiClientDms>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

/// TODO : Documentation
#[derive(Debug, Clone)]
pub struct DmsCapabilities {
    pub max_tx_channel_rate: u32,
    pub max_rx_channel_rate: u32,
    pub data_service_capability: DmsDataServiceCapability,
    pub sim_capability: DmsSimCapability,
    pub radio_interfaces: Vec<DmsRadioInterface>,
}

client_drop_impl!(DmsClient);

client_clone_impl!(DmsClient);

impl DmsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(DmsClient, QmiClientDms, libqmi_glib_types_rs::Service::Dms);

    close_client_impl!("dms");

    pub async fn get_capabilities(&self) -> QmiResult<DmsCapabilities> {
        let (tx, rx) = oneshot::channel::<QmiResult<DmsCapabilities>>();

        unsafe {
            qmi_client_dms_get_capabilities(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_capabilities_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }
}

allocate_client_callback!(allocate_client_cb, QmiClientDms, "allocate DMS client");

release_client_callback!(release_client_cb, "release DMS client");

unsafe extern "C" fn get_capabilities_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<DmsCapabilities>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_dms_get_capabilities_finish(client.cast(), res, &mut error);
    let mut output = || -> QmiResult<DmsCapabilities> {
        if output.is_null() {
            return Err((error, "get_capabilities_cb finish").into());
        }

        let ret = { qmi_message_dms_get_capabilities_output_get_result(output, &mut error) };
        if ret == GFALSE {
            qmi_message_dms_get_capabilities_output_unref(output);
            return Err((error, "get_capabilities_cb output").into());
        }

        let mut value_info_max_tx_channel_rate: u32 = 0;
        let mut value_info_max_rx_channel_rate: u32 = 0;
        let mut value_info_data_service_capability = DmsDataServiceCapability::None.into_glib();
        let mut value_info_sim_capability = DmsSimCapability::NotSupported.into_glib();
        let mut value_info_radio_interface_list: *mut GArray = null_mut();
        let ret = qmi_message_dms_get_capabilities_output_get_info(
            output,
            &mut value_info_max_tx_channel_rate,
            &mut value_info_max_rx_channel_rate,
            &mut value_info_data_service_capability,
            &mut value_info_sim_capability,
            &mut value_info_radio_interface_list,
            &mut error,
        );
        if ret == GFALSE || value_info_radio_interface_list.is_null() {
            qmi_message_dms_get_capabilities_output_unref(output);
            return Err((error, "get_capabilities_cb get_info").into());
        }

        let slice = std::slice::from_raw_parts(
            (*value_info_radio_interface_list)
                .data
                .cast::<QmiDmsRadioInterface>(),
            (*value_info_radio_interface_list).len as usize,
        );
        let result = DmsCapabilities {
            max_tx_channel_rate: value_info_max_tx_channel_rate,
            max_rx_channel_rate: value_info_max_rx_channel_rate,
            data_service_capability: from_glib(value_info_data_service_capability),
            sim_capability: from_glib(value_info_sim_capability),
            radio_interfaces: slice.iter().map(|iface| from_glib(*iface)).collect(),
        };
        qmi_message_dms_get_capabilities_output_unref(output);
        Ok(result)
    };
    drop(sender.send(output()).inspect_err(cannot_send));
}
