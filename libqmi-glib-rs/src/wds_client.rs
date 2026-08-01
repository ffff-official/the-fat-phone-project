pub use libqmi_glib_types_rs::{WdsConnectionStatus, WdsIpFamily};

use super::*;

use crate::sys::*;
use glib_sys::{GArray, GError, GFALSE, gboolean};
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use std::cell::RefCell;
use std::ffi::CString;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

#[cfg(feature = "v1_32")]
use {
    libqmi_glib_types_rs::{WdsPdpType, WdsRequestedSettings},
    std::net::{Ipv4Addr, Ipv6Addr},
};

/// A client for interacting with the Wireless Data Service (WDS) of a QMI device.
///
/// [`WdsClient`] provides asynchronous operations for managing packet data
/// connections through a QMI-compatible modem.
///
/// It can be used to:
///
/// - establish and terminate mobile data connections;
/// - monitor packet service status;
/// - retrieve connection parameters and network settings;.
///
/// The underlying QMI client is reference-counted and remains valid while at
/// least one [`WdsClient`] instance exists.
#[derive(Debug)]
pub struct WdsClient {
    client: NonNull<QmiClientWds>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(WdsClient);

client_clone_impl!(WdsClient);

impl WdsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(WdsClient, QmiClientWds, libqmi_glib_types_rs::Service::Wds);

    close_client_impl!("wds");
}

allocate_client_callback!(allocate_client_cb, QmiClientWds, "allocate WDS client");

release_client_callback!(release_client_cb, "release WDS client");

type Context = WdsClient;

getter!(
    /// Asynchronously sends a Get Packet Service Status request to the device.
    get_packet_service_status,
    qmi_message_wds_get_packet_service_status_output_get_connection_status,
    WdsConnectionStatus,
    qmi_client_wds_get_packet_service_status,
    qmi_client_wds_get_packet_service_status_finish,
    qmi_message_wds_get_packet_service_status_output_get_result,
    qmi_message_wds_get_packet_service_status_output_unref,
    "get_packet_service_status"
);

/// Current Ipv4 settings.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Ipv4NetworkSettings {
    pub address: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub netmask: Option<Ipv4Addr>,
    pub primary_dns: Option<Ipv4Addr>,
    pub secondary_dns: Option<Ipv4Addr>,
}

/// Current Ipv6 settings.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Ipv6NetworkSettings {
    pub address: Option<Ipv6Addr>,
    pub prefix_length: Option<u8>,
    pub gateway: Option<Ipv6Addr>,
    pub gateway_prefix_length: Option<u8>,
    pub primary_dns: Option<Ipv6Addr>,
    pub secondary_dns: Option<Ipv6Addr>,
}

/// Current IP settings.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct NetworkSettings {
    pub apn: Option<String>,
    pub mtu: Option<u32>,
    pub pdp_type: Option<WdsPdpType>,

    pub ipv4: Ipv4NetworkSettings,
    pub ipv6: Ipv6NetworkSettings,
}

#[cfg(feature = "v1_32")]
unsafe fn get_current_ip_settings_writer(
    input: *mut QmiMessageWdsGetCurrentSettingsInput,
    error: *mut *mut GError,
    settings: WdsRequestedSettings,
) -> gboolean {
    qmi_message_wds_get_current_settings_input_set_requested_settings(
        input,
        settings.into_glib(),
        error,
    )
}

#[cfg(feature = "v1_32")]
unsafe fn get_current_ip_settings_extractor(
    output: *mut QmiMessageWdsGetCurrentSettingsOutput,
    _error: *mut *mut GError,
) -> Result<NetworkSettings, ()> {
    let mtu = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_mtu(output, value, error)
    });
    let pdp = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_pdp_type(output, value, error)
    });
    let apn = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_apn_name(output, value, error)
    });
    let ipv4_address = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_ipv4_address(output, value, error)
    });
    let ipv4_gateway = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_ipv4_gateway_address(output, value, error)
    });
    let ipv4_netmask = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_ipv4_gateway_subnet_mask(
            output, value, error,
        )
    });
    let ipv4_primary_dns = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_primary_ipv4_dns_address(
            output, value, error,
        )
    });
    let ipv4_secondary_dns = get_optional(|value, error| {
        qmi_message_wds_get_current_settings_output_get_secondary_ipv4_dns_address(
            output, value, error,
        )
    });
    let ipv6_addr = get_optional_ipv6_with_prefix_len(|value, len, error| {
        qmi_message_wds_get_current_settings_output_get_ipv6_address(output, value, len, error)
    });
    let ipv6_gw = get_optional_ipv6_with_prefix_len(|value, len, error| {
        qmi_message_wds_get_current_settings_output_get_ipv6_gateway_address(
            output, value, len, error,
        )
    });
    let ipv6_primary_dns = get_optional_ipv6(|value, error| {
        qmi_message_wds_get_current_settings_output_get_ipv6_primary_dns_address(
            output, value, error,
        )
    });
    let ipv6_secondary_dns = get_optional_ipv6(|value, error| {
        qmi_message_wds_get_current_settings_output_get_ipv6_secondary_dns_address(
            output, value, error,
        )
    });
    let apn = apn
        .filter(|value| !value.is_null())
        .map(|s| crate::c_char_to_string(s));
    Ok(NetworkSettings {
        pdp_type: pdp.map(|pdp| from_glib(pdp)),
        mtu,
        apn,
        ipv4: Ipv4NetworkSettings {
            address: ipv4_address.map(Ipv4Addr::from),
            gateway: ipv4_gateway.map(Ipv4Addr::from),
            netmask: ipv4_netmask.map(Ipv4Addr::from),
            primary_dns: ipv4_primary_dns.map(Ipv4Addr::from),
            secondary_dns: ipv4_secondary_dns.map(Ipv4Addr::from),
        },
        ipv6: Ipv6NetworkSettings {
            address: ipv6_addr.map(|ipv6| ipv6.0),
            prefix_length: ipv6_addr.map(|ipv6| ipv6.1),
            gateway: ipv6_gw.map(|ipv6| ipv6.0),
            gateway_prefix_length: ipv6_gw.map(|ipv6| ipv6.1),
            primary_dns: ipv6_primary_dns,
            secondary_dns: ipv6_secondary_dns,
        },
    })
}

#[cfg(feature = "v1_32")]
unsafe fn get_optional<F, T: Default>(mut f: F) -> Option<T>
where
    F: FnMut(*mut T, *mut *mut GError) -> gboolean,
{
    let mut value: T = T::default();

    match f(&mut value, null_mut()) {
        GFALSE => None,
        _ => Some(value),
    }
}

#[cfg(feature = "v1_32")]
unsafe fn get_optional_ipv6<F>(mut getter: F) -> Option<Ipv6Addr>
where
    F: FnMut(*mut *mut GArray, *mut *mut GError) -> gboolean,
{
    let mut array: *mut GArray = null_mut();

    if getter(&mut array, null_mut()) == GFALSE || array.is_null() {
        return None;
    }

    if (*array).len != 16 {
        return None;
    }

    let bytes = std::slice::from_raw_parts((*array).data.cast::<u8>(), 16);

    let mut address = [0_u8; 16];
    address.copy_from_slice(bytes);

    Some(Ipv6Addr::from(address))
}

#[cfg(feature = "v1_32")]
unsafe fn get_optional_ipv6_with_prefix_len<F>(mut getter: F) -> Option<(Ipv6Addr, u8)>
where
    F: FnMut(*mut *mut GArray, *mut u8, *mut *mut GError) -> gboolean,
{
    let mut array: *mut GArray = null_mut();
    let mut prefix_len: u8 = 0;

    if getter(&mut array, &mut prefix_len, null_mut()) == GFALSE || array.is_null() {
        return None;
    }

    if (*array).len != 16 {
        return None;
    }

    let bytes = std::slice::from_raw_parts((*array).data.cast::<u8>(), 16);

    let mut address = [0_u8; 16];
    address.copy_from_slice(bytes);

    Some((Ipv6Addr::from(address), prefix_len))
}

request_with_writer_and_extractor!(
    /// Asynchronously sends a Get Current ip Settings request to the device.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    get_current_ip_settings,
    get_current_ip_settings_writer,
    settings: WdsRequestedSettings,
    qmi_message_wds_get_current_settings_input_new,
    qmi_message_wds_get_current_settings_input_unref,
    qmi_client_wds_get_current_settings,
    qmi_client_wds_get_current_settings_finish,
    qmi_message_wds_get_current_settings_output_get_result,
    qmi_message_wds_get_current_settings_output_unref,
    NetworkSettings,
    get_current_ip_settings_extractor,
    "get_current_ip_settings"
);

unsafe fn start_ipv4_network_writer(
    input: *mut QmiMessageWdsStartNetworkInput,
    error: *mut *mut GError,
    apn: impl AsRef<str>,
) -> gboolean {
    let c_string = CString::new(apn.as_ref()).expect("APN contains a NULL byte");
    let ret = qmi_message_wds_start_network_input_set_apn(input, c_string.as_ptr(), error);
    if ret == GFALSE {
        return ret;
    }
    qmi_message_wds_start_network_input_set_ip_family_preference(
        input,
        WdsIpFamily::Ipv4.into_glib(),
        error,
    )
}

unsafe fn start_ipv4_network_extractor(
    output: *mut QmiMessageWdsStartNetworkOutput,
    error: *mut *mut GError,
) -> Result<u32, ()> {
    let mut pdh = 0;
    let ret = qmi_message_wds_start_network_output_get_packet_data_handle(output, &mut pdh, error);
    match ret {
        GFALSE => Err(()),
        _ => Ok(pdh),
    }
}

request_with_writer_and_extractor!(
    // Asynchronously sends a Start Network request to the device.
    start_ipv4_network,
    start_ipv4_network_writer,
    apn: impl AsRef<str>,
    qmi_message_wds_start_network_input_new,
    qmi_message_wds_start_network_input_unref,
    qmi_client_wds_start_network,
    qmi_client_wds_start_network_finish,
    qmi_message_wds_start_network_output_get_result,
    qmi_message_wds_start_network_output_unref,
    u32,
    start_ipv4_network_extractor,
    "start_ipv4_network"
);

unsafe fn stop_ipv4_network_writer(
    input: *mut QmiMessageWdsStopNetworkInput,
    error: *mut *mut GError,
    pdh: u32,
) -> gboolean {
    qmi_message_wds_stop_network_input_set_packet_data_handle(input, pdh, error)
}

setter_with_writer!(
    /// Stops the packet data session identified by the supplied handle.
    stop_ipv4_network,
    stop_ipv4_network_writer,
    pdh: u32,
    qmi_message_wds_stop_network_input_new,
    qmi_message_wds_stop_network_input_unref,
    qmi_client_wds_stop_network,
    qmi_client_wds_stop_network_finish,
    qmi_message_wds_stop_network_output_get_result,
    qmi_message_wds_stop_network_output_unref,
    "stop_ipv4_network"
);
