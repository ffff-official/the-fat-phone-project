use super::*;

use crate::sys::*;
use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GError, GFALSE, gboolean, guint32};
use gobject_sys::{GObject, g_object_ref, g_object_unref};
use libqmi_glib_types_rs::glib::translate::FromGlib;
use libqmi_glib_types_rs::glib::translate::IntoGlib;
use libqmi_glib_types_rs::{
    DeviceReleaseClientFlags, Service, WdsConnectionStatus, WdsIpFamily, WdsPdpType,
    WdsRequestedSettings,
};

use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::fmt;
use std::net::Ipv4Addr;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

/// A client for interacting with the Wireless Data Service (WDS) interface of a QMI device.
#[derive(Debug)]
pub struct WdsClient {
    client: NonNull<QmiClientWds>,
    device: Device,
    pdh: Rc<RefCell<Option<u32>>>, // session identifier
}

impl Drop for WdsClient {
    fn drop(&mut self) {
        tracing::trace!("WDS CLIENT is droped !");
        unsafe {
            g_object_unref(self.client.as_ptr() as _);
        }
    }
}

impl Clone for WdsClient {
    fn clone(&self) -> Self {
        unsafe {
            g_object_ref(self.client.as_ptr() as _);
        }
        Self {
            client: self.client,
            device: self.device.clone(),
            pdh: self.pdh.clone(),
        }
    }
}

impl WdsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    /// Asynchronously allocates a new [`WdsClient`].
    pub async fn new(device: &Device) -> QmiResult<WdsClient> {
        let (tx, rx) = oneshot::channel::<QmiResult<NonNull<QmiClientWds>>>();

        unsafe {
            qmi_device_allocate_client(
                device.as_ptr(),
                Service::Wds.into_glib(),
                QMI_CID_NONE as _,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(allocate_client_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            )
        };
        match rx.await.map_err(|_| QmiError::cb_droped())? {
            Ok(client) => Ok(Self {
                device: device.clone(),
                client,
                pdh: Rc::new(RefCell::new(None)),
            }),
            Err(e) => Err(e),
        }
    }

    /// Asynchronously releases the [`WdsClient`] from the [`Device`].
    pub async fn close(self) -> QmiResult<()> {
        let ref_count = unsafe { *(self.client.as_ptr() as *mut GObject) }.ref_count;
        if ref_count != 2 {
            Err(QmiError::new(
                format!("can't close ref count = {}", ref_count),
                "wds close",
            ))?;
        }
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

        unsafe {
            qmi_device_release_client(
                self.device.as_ptr(),
                self.client.as_ptr() as _,
                DeviceReleaseClientFlags::RELEASE_CID.into_glib(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(release_client_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            );
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously sends a Start Network request to the device.
    pub async fn start_network(&mut self, apn: &str) -> QmiResult<()> {
        let (tx, rx) = oneshot::channel::<QmiResult<u32>>();
        let c_string =
            CString::new(apn).map_err(|e| Into::<QmiError>::into((e, "start network new", ())))?;
        let input = unsafe { qmi_message_wds_start_network_input_new() };
        if input.is_null() {
            Err(QmiError::new("nullptr input", "start_network"))?;
        }
        let mut error: *mut GError = null_mut();

        let ret = unsafe {
            qmi_message_wds_start_network_input_set_apn(input, c_string.as_ptr(), &mut error)
        };
        if ret == GFALSE {
            Err((error, "start network set apn"))?
        } else {
            let ret = unsafe {
                qmi_message_wds_start_network_input_set_ip_family_preference(
                    input,
                    WdsIpFamily::Ipv4.into_glib(),
                    &mut error,
                )
            };
            if ret == GFALSE {
                Err((error, "start network set ip family"))?
            } else {
                unsafe {
                    qmi_client_wds_start_network(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(start_network_cb),
                        Box::into_raw(Box::new(tx)) as *mut c_void,
                    );
                    qmi_message_wds_start_network_input_unref(input);
                }
                match rx.await {
                    Ok(pdh) => {
                        *self.pdh.borrow_mut() = Some(pdh?);
                        Ok(())
                    }
                    Err(_e) => Err(QmiError::cb_droped()),
                }
            }
        }
    }

    /// Asynchronously sends a Stop Network request to the device.
    pub async fn stop_network(&mut self) -> QmiResult<()> {
        let pdh = self.pdh.borrow().ok_or_else(|| QmiError {
            message: "Got not PDH : Network seems to be already stoped !".into(),
            context: "stop network",
        })?;
        let (tx, rx) = oneshot::channel::<QmiResult<()>>();
        let input = unsafe { qmi_message_wds_stop_network_input_new() };
        let mut error: *mut GError = null_mut();

        let ret = unsafe {
            qmi_message_wds_stop_network_input_set_packet_data_handle(input, pdh, &mut error)
        };
        if ret == GFALSE {
            unsafe {
                qmi_message_wds_stop_network_input_unref(input);
            }
            Err((error, "stop network set pdh"))?
        } else {
            unsafe {
                qmi_client_wds_stop_network(
                    self.client.as_ptr(),
                    input,
                    OPERATION_TIMEOUT,
                    null_mut(),
                    Some(stop_network_cb),
                    Box::into_raw(Box::new(tx)) as *mut c_void,
                );
            }
            let ret = rx.await;
            unsafe {
                qmi_message_wds_stop_network_input_unref(input);
            }
            ret.map_err(|_| QmiError::cb_droped())?.map(|_| {
                *self.pdh.borrow_mut() = None;
            })
        }
    }

    /// Asynchronously sends a Get Packet Service Status request to the device.
    pub async fn get_packet_service_status(&self) -> QmiResult<WdsConnectionStatus> {
        let (tx, rx) = oneshot::channel::<QmiResult<WdsConnectionStatus>>();

        unsafe {
            qmi_client_wds_get_packet_service_status(
                self.client.as_ptr(),
                null_mut(),
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_packet_service_status_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            )
        }
        rx.await.map_err(|_| QmiError::cb_droped())?
    }

    /// Asynchronously sends a Get Current ipv4 Settings request to the device.
    pub async fn get_current_ipv4_settings(&self) -> QmiResult<Ipv4NetworkSettings> {
        let (tx, rx) = oneshot::channel::<QmiResult<Ipv4NetworkSettings>>();
        let input = unsafe { qmi_message_wds_get_current_settings_input_new() };
        if input.is_null() {
            Err(QmiError::new("nullptr input", "get_current_settings"))?;
        }
        let mut error: *mut GError = null_mut();
        use WdsRequestedSettings as Q;
        let settings = Q::IP_ADDRESS | Q::GATEWAY_INFO | Q::PDP_TYPE | Q::MTU;

        let ret = unsafe {
            qmi_message_wds_get_current_settings_input_set_requested_settings(
                input,
                settings.into_glib(),
                &mut error,
            )
        };
        if ret == GFALSE {
            unsafe { qmi_message_wds_get_current_settings_input_unref(input) }
            Err((error, "get_current_settings"))?
        }

        unsafe {
            qmi_client_wds_get_current_settings(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(get_current_settings_cb),
                Box::into_raw(Box::new(tx)) as *mut c_void,
            );
        }
        let ret = rx.await;
        unsafe {
            qmi_message_wds_get_current_settings_input_unref(input);
        }
        ret.map_err(|_| QmiError::cb_droped())?
    }
}

unsafe extern "C" fn allocate_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<NonNull<QmiClientWds>>>);
    let mut error: *mut GError = null_mut();

    let client = qmi_device_allocate_client_finish(device as *mut QmiDevice, res, &mut error);

    let output = match NonNull::new(client as _) {
        None => Err((error, "new wds client").into()),
        Some(client) => Ok(client),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn release_client_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>);
    let mut error: *mut GError = null_mut();

    let ret = qmi_device_release_client_finish(device as *mut QmiDevice, res, &mut error);

    let output = match ret {
        GFALSE => Err((error, "release wds client").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn start_network_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<u32>>);
    let mut error: *mut GError = null_mut();

    let output = qmi_client_wds_start_network_finish(device as _, res, &mut error);
    let output = match output.is_null() {
        true => Err((error, "start network cb finish").into()),
        false => {
            let mut pdh: u32 = 0;
            let ret = qmi_message_wds_start_network_output_get_packet_data_handle(
                output, &mut pdh, &mut error,
            );
            qmi_message_wds_start_network_output_unref(output);
            match ret {
                GFALSE => Err((error, "start network cb pdh").into()),
                _ => Ok(pdh),
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn stop_network_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<()>>);
    let mut error: *mut GError = null_mut();

    let output = qmi_client_wds_stop_network_finish(device as _, res, &mut error);
    let output = match output.is_null() {
        true => Err((error, "stop network cb finish").into()),
        false => {
            let ret = qmi_message_wds_stop_network_output_get_result(output, &mut error);
            qmi_message_wds_stop_network_output_unref(output);
            match ret {
                GFALSE => Err((error, "stop network get result").into()),
                _ => Ok(()),
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn get_packet_service_status_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<WdsConnectionStatus>>);
    let mut error: *mut GError = null_mut();

    let status_output =
        qmi_client_wds_get_packet_service_status_finish(device as _, res, &mut error);
    let output = match status_output.is_null() {
        true => Err((error, "packet service status cb get").into()),
        false => {
            let mut value_connection_status = 0;
            let status = qmi_message_wds_get_packet_service_status_output_get_connection_status(
                status_output,
                &mut value_connection_status,
                &mut error,
            );
            qmi_message_wds_get_packet_service_status_output_unref(status_output);
            match status {
                GFALSE => Err((error, "packet service status cb").into()),
                _ => Ok(WdsConnectionStatus::from_glib(value_connection_status)),
            }
        }
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn get_current_settings_cb(
    device: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data as *mut oneshot::Sender<QmiResult<Ipv4NetworkSettings>>);
    let mut error: *mut GError = null_mut();
    let status_output = qmi_client_wds_get_current_settings_finish(device as _, res, &mut error);
    let ipv4_network_settings = match status_output.is_null() {
        true => Err((error, "get_current_settings_cb").into()),
        false => {
            let ipv4_network_settings = Ipv4NetworkSettings {
                ipv4: get_optional_u32("Cant get ipv4", |value, error| {
                    qmi_message_wds_get_current_settings_output_get_ipv4_address(
                        status_output,
                        value,
                        error,
                    )
                }),
                gw: get_optional_u32("Cant get gw", |value, error| {
                    qmi_message_wds_get_current_settings_output_get_ipv4_gateway_address(
                        status_output,
                        value,
                        error,
                    )
                }),
                mask: get_optional_u32("Cant get mask", |value, error| {
                    qmi_message_wds_get_current_settings_output_get_ipv4_gateway_subnet_mask(
                        status_output,
                        value,
                        error,
                    )
                }),
                pdp: get_optional_u32("Cant get pdp", |value, error| {
                    qmi_message_wds_get_current_settings_output_get_pdp_type(
                        status_output,
                        value as _,
                        error,
                    )
                })
                .map(|m| WdsPdpType::from_glib(m as i32)),
                mtu: get_optional_u32("Cant get mtu", |value, error| {
                    qmi_message_wds_get_current_settings_output_get_mtu(status_output, value, error)
                }),
            };
            qmi_message_wds_get_current_settings_output_unref(status_output);
            Ok(ipv4_network_settings)
        }
    };
    drop(sender.send(ipv4_network_settings).inspect_err(cannot_send));
}

unsafe fn get_optional_u32<F>(label: &'static str, mut f: F) -> Option<guint32>
where
    F: FnMut(*mut guint32, *mut *mut GError) -> gboolean,
{
    let mut value: guint32 = 0;
    let mut error: *mut GError = std::ptr::null_mut();

    match f(&mut value, &mut error) {
        GFALSE => {
            tracing::warn!("{:?}", Into::<QmiError>::into((error, label)));
            None
        }
        _ => Some(value),
    }
}

/// Current Ipv4 settings.
#[derive(Debug, Default)]
pub struct Ipv4NetworkSettings {
    pub ipv4: Option<guint32>,
    pub gw: Option<guint32>,
    pub mask: Option<guint32>,
    pub mtu: Option<guint32>,
    pub pdp: Option<WdsPdpType>,
}

impl fmt::Display for Ipv4NetworkSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "IPv4 network settings:")?;
        let ipv4 = self.ipv4.map(|x| x.to_be_bytes()).unwrap_or([0, 0, 0, 0]);
        let mask = self.mask.map(|x| x.to_be_bytes()).unwrap_or([0, 0, 0, 0]);
        let gw = self.gw.map(|x| x.to_be_bytes()).unwrap_or([0, 0, 0, 0]);
        let mtu = self.mtu.map(|x| format!("{x}")).unwrap_or("NIA".into());
        let pdp = self.pdp.map(|x| format!("{x:?}")).unwrap_or("NIA".into());

        writeln!(f, "  Address : {}", Ipv4Addr::from(ipv4))?;
        writeln!(f, "  Netmask : {}", Ipv4Addr::from(mask))?;
        writeln!(f, "  Gateway : {}", Ipv4Addr::from(gw))?;
        writeln!(f, "  MTU     : {}", mtu)?;
        write!(f, "  PDP     : {}", pdp)?;
        Ok(())
    }
}
