mod packet_service_status;
pub use packet_service_status::WdsPacketServiceStatus;

#[cfg(feature = "v1_18")]
pub use libqmi_glib_types_rs::WdsDormancyStatus;
#[cfg(feature = "v1_6")]
pub use libqmi_glib_types_rs::WdsPacketStatisticsMaskFlag;
pub use libqmi_glib_types_rs::{
    WdsConnectionStatus, WdsDataBearerTechnology, WdsIpFamily, WdsNetworkType,
};
#[cfg(feature = "v1_32")]
pub use libqmi_glib_types_rs::{WdsPdpType, WdsRequestedSettings};

use super::*;
use crate::sys::*;

use glib_sys::{GArray, GError, GFALSE, GTRUE, gboolean, guint8};
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use std::cell::RefCell;
use std::ffi::CString;
use std::ptr::{NonNull, null_mut};
use std::rc::Rc;

#[cfg(feature = "v1_32")]
use std::net::{Ipv4Addr, Ipv6Addr};

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
    /// Retrieves the current packet data service status.
    ///
    /// The returned status indicates whether the packet data service is currently
    /// connected, disconnected, suspended, or in another modem-defined state.
    get_packet_service_status,
    qmi_message_wds_get_packet_service_status_output_get_connection_status,
    WdsConnectionStatus,
    qmi_client_wds_get_packet_service_status,
    qmi_client_wds_get_packet_service_status_finish,
    qmi_message_wds_get_packet_service_status_output_get_result,
    qmi_message_wds_get_packet_service_status_output_unref,
    "get_packet_service_status"
);

/// Current IPv4 network configuration reported by the modem.
///
/// All fields are optional because the modem may omit individual settings,
/// depending on the requested information and the active packet data session.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Ipv4NetworkSettings {
    /// Assigned IPv4 address.
    pub address: Option<Ipv4Addr>,
    /// Default IPv4 gateway.
    pub gateway: Option<Ipv4Addr>,
    /// IPv4 subnet mask.
    pub netmask: Option<Ipv4Addr>,
    /// Primary IPv4 DNS server.
    pub primary_dns: Option<Ipv4Addr>,
    /// Secondary IPv4 DNS server.
    pub secondary_dns: Option<Ipv4Addr>,
}

/// Current IPv6 network configuration reported by the modem.
///
/// All fields are optional because the modem may omit individual settings,
/// depending on the requested information and the active packet data session.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Ipv6NetworkSettings {
    /// Assigned IPv6 address.
    pub address: Option<Ipv6Addr>,
    /// Prefix length of the assigned IPv6 address.
    pub prefix_length: Option<u8>,
    /// Default IPv6 gateway.
    pub gateway: Option<Ipv6Addr>,
    /// Prefix length of the IPv6 gateway.
    pub gateway_prefix_length: Option<u8>,
    /// Primary IPv6 DNS server.
    pub primary_dns: Option<Ipv6Addr>,
    /// Secondary IPv6 DNS server.
    pub secondary_dns: Option<Ipv6Addr>,
}

/// Current network configuration reported by the modem.
///
/// The returned settings describe the active packet data session, including
/// general session parameters and the IPv4 and IPv6 network configuration.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct NetworkSettings {
    /// Access Point Name (APN) used by the current packet data session.
    pub apn: Option<String>,
    /// Maximum Transmission Unit (MTU) of the current packet data session.
    pub mtu: Option<u32>,
    /// Packet Data Protocol (PDP) type of the current packet data session.
    pub pdp_type: Option<WdsPdpType>,
    /// Current IPv4 network configuration.
    pub ipv4: Ipv4NetworkSettings,
    /// Current IPv6 network configuration.
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
    /// Retrieves the current network settings reported by the modem.
    ///
    /// The supplied flags specify which settings should be requested. The
    /// returned value may include general session parameters together with
    /// the current IPv4 and IPv6 configuration.
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
    /// Starts an IPv4 packet data session.
    ///
    /// The supplied Access Point Name (APN) identifies the packet data
    /// network to connect to.
    ///
    /// On success, returns the packet data handle (PDH) identifying the
    /// newly established session.
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
    /// Stops the IPv4 packet data session identified by the supplied
    /// packet data handle (PDH).
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

unsafe fn get_supported_messages_extractor(
    output: *mut QmiMessageWdsGetSupportedMessagesOutput,
    error: *mut *mut GError,
    param: &mut Vec<u8>,
) -> gboolean {
    let mut value_list: *mut GArray = null_mut();

    let ret =
        qmi_message_wds_get_supported_messages_output_get_list(output, &mut value_list, error);
    if ret == GTRUE {
        *param = garray_primitive_to_vec::<guint8, _>(value_list);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the bitmap of WDS messages supported by the modem.
    ///
    /// Each bit in the returned byte vector corresponds to a WDS message
    /// identifier. A set bit indicates that the modem reports support for
    /// that message.
    ///
    /// The message identifier associated with a bit is calculated as:
    ///
    /// `byte_index * 8 + bit_index`
    #[cfg(feature = "v1_14")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
    get_supported_messages,
    get_supported_messages_extractor,
    Vec<u8>,
    qmi_client_wds_get_supported_messages,
    qmi_client_wds_get_supported_messages_finish,
    qmi_message_wds_get_supported_messages_output_get_result,
    qmi_message_wds_get_supported_messages_output_unref,
    "get_supported_messages"
);

#[cfg(feature = "v1_6")]
/// Packet transfer statistics reported by the WDS service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WdsPacketStatistics {
    /// Number of successfully transmitted packets.
    pub tx_packets_ok: Option<u32>,
    /// Number of successfully received packets.
    pub rx_packets_ok: Option<u32>,
    /// Number of packets that could not be transmitted.
    pub tx_packets_error: Option<u32>,
    /// Number of received packets containing errors.
    pub rx_packets_error: Option<u32>,
    /// Number of transmit buffer overflows.
    pub tx_overflows: Option<u32>,
    /// Number of receive buffer overflows.
    pub rx_overflows: Option<u32>,
    /// Number of successfully transmitted bytes.
    pub tx_bytes_ok: Option<u64>,
    /// Number of successfully received bytes.
    pub rx_bytes_ok: Option<u64>,
}

#[cfg(feature = "v1_6")]
unsafe fn get_packet_statistics_writer(
    input: *mut QmiMessageWdsGetPacketStatisticsInput,
    error: *mut *mut GError,
    mask: WdsPacketStatisticsMaskFlag,
) -> gboolean {
    qmi_message_wds_get_packet_statistics_input_set_mask(input, mask.into_glib(), error)
}

#[cfg(feature = "v1_6")]
unsafe fn get_packet_statistics_extractor(
    output: *mut QmiMessageWdsGetPacketStatisticsOutput,
    _error: *mut *mut GError,
) -> Result<WdsPacketStatistics, ()> {
    let tx_packets_ok = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_tx_packets_ok(output, value, error)
    });
    let rx_packets_ok = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_rx_packets_ok(output, value, error)
    });
    let tx_packets_error = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_tx_packets_error(output, value, error)
    });
    let rx_packets_error = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_rx_packets_error(output, value, error)
    });
    let tx_overflows = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_tx_overflows(output, value, error)
    });
    let rx_overflows = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_rx_overflows(output, value, error)
    });
    let tx_bytes_ok = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_tx_bytes_ok(output, value, error)
    });
    let rx_bytes_ok = get_optional(|value, error| {
        qmi_message_wds_get_packet_statistics_output_get_rx_bytes_ok(output, value, error)
    });
    Ok(WdsPacketStatistics {
        tx_packets_ok,
        rx_packets_ok,
        tx_packets_error,
        rx_packets_error,
        tx_overflows,
        rx_overflows,
        tx_bytes_ok,
        rx_bytes_ok,
    })
}

request_with_writer_and_extractor!(
    /// Retrieves packet transfer statistics from the WDS service.
    ///
    /// The supplied mask specifies which counters should be included in
    /// the response.
    #[cfg(feature = "v1_6")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_6")))]
    get_packet_statistics,
    get_packet_statistics_writer,
    statistics: WdsPacketStatisticsMaskFlag,
    qmi_message_wds_get_packet_statistics_input_new,
    qmi_message_wds_get_packet_statistics_input_unref,
    qmi_client_wds_get_packet_statistics,
    qmi_client_wds_get_packet_statistics_finish,
    qmi_message_wds_get_packet_statistics_output_get_result,
    qmi_message_wds_get_packet_statistics_output_unref,
    WdsPacketStatistics,
    get_packet_statistics_extractor,
    "get_packet_statistics"
);

getter!(
    /// Retrieves the data bearer technology used by the current packet
    /// data session.
    get_data_bearer_technology,
    qmi_message_wds_get_data_bearer_technology_output_get_current,
    WdsDataBearerTechnology,
    qmi_client_wds_get_data_bearer_technology,
    qmi_client_wds_get_data_bearer_technology_finish,
    qmi_message_wds_get_data_bearer_technology_output_get_result,
    qmi_message_wds_get_data_bearer_technology_output_unref,
    "get_data_bearer_technology"
);

/// Detailed bearer technology information for the current data session.
///
/// This structure describes the radio technology currently used by the active
/// data bearer.
///
/// The meaning of `rat_mask` and `service_option_mask` depends on the selected
/// [`WdsNetworkType`]. These fields are bitmasks defined by the QMI WDS
/// protocol and identify the active radio access technology (RAT) and any
/// additional service options reported by the modem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WdsDetailedBearerTechnology {
    /// The network family associated with the current bearer.
    pub network_type: WdsNetworkType,
    /// Bitmask describing the active radio access technology (RAT).
    ///
    /// The interpretation of this field depends on [`Self::network_type`].
    pub rat_mask: u32,
    /// Bitmask describing additional service options for the current bearer.
    ///
    /// The interpretation of this field depends on [`Self::network_type`].
    pub service_option_mask: u32,
}

impl Default for WdsDetailedBearerTechnology {
    fn default() -> Self {
        Self {
            network_type: WdsNetworkType::__Unknown(0),
            rat_mask: u32::default(),
            service_option_mask: u32::default(),
        }
    }
}

unsafe fn get_current_data_bearer_technology_extractor(
    output: *mut QmiMessageWdsGetCurrentDataBearerTechnologyOutput,
    error: *mut *mut GError,
    bearer_tech: &mut WdsDetailedBearerTechnology,
) -> gboolean {
    let mut value_current_network_type: QmiWdsNetworkType = 0;
    let ret = qmi_message_wds_get_current_data_bearer_technology_output_get_current(
        output,
        &mut value_current_network_type,
        &mut bearer_tech.rat_mask,
        &mut bearer_tech.service_option_mask,
        error,
    );
    if ret == GTRUE {
        bearer_tech.network_type = from_glib(value_current_network_type);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves detailed bearer technology information for the current
    /// packet data session.
    get_current_data_bearer_technology,
    get_current_data_bearer_technology_extractor,
    WdsDetailedBearerTechnology,
    qmi_client_wds_get_current_data_bearer_technology,
    qmi_client_wds_get_current_data_bearer_technology_finish,
    qmi_message_wds_get_current_data_bearer_technology_output_get_result,
    qmi_message_wds_get_current_data_bearer_technology_output_unref,
    "get_current_data_bearer_technology"
);

unsafe fn get_last_data_bearer_technology_extractor(
    output: *mut QmiMessageWdsGetCurrentDataBearerTechnologyOutput,
    error: *mut *mut GError,
    bearer_tech: &mut WdsDetailedBearerTechnology,
) -> gboolean {
    let mut value_last_network_type: QmiWdsNetworkType = 0;
    let ret = qmi_message_wds_get_current_data_bearer_technology_output_get_last(
        output,
        &mut value_last_network_type,
        &mut bearer_tech.rat_mask,
        &mut bearer_tech.service_option_mask,
        error,
    );
    if ret == GTRUE {
        bearer_tech.network_type = from_glib(value_last_network_type);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves detailed bearer technology information for the last
    /// packet data session.
    get_last_data_bearer_technology,
    get_last_data_bearer_technology_extractor,
    WdsDetailedBearerTechnology,
    qmi_client_wds_get_current_data_bearer_technology,
    qmi_client_wds_get_current_data_bearer_technology_finish,
    qmi_message_wds_get_current_data_bearer_technology_output_get_result,
    qmi_message_wds_get_current_data_bearer_technology_output_unref,
    "get_last_data_bearer_technology"
);

getter!(
    /// Retrieves the dormancy status of the current packet data session.
    #[cfg(feature = "v1_18")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_18")))]
    get_dormancy_status,
    qmi_message_wds_get_dormancy_status_output_get_dormancy_status,
    WdsDormancyStatus,
    qmi_client_wds_get_dormancy_status,
    qmi_client_wds_get_dormancy_status_finish,
    qmi_message_wds_get_dormancy_status_output_get_result,
    qmi_message_wds_get_dormancy_status_output_unref,
    "get_dormancy_status"
);
