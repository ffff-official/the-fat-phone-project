use super::*;

pub use libqmi_glib_types_rs::WdsCallEndReason;
/// Callback invoked when the modem reports a packet data connection status change.
pub type PacketServiceStatusCallback = Box<dyn FnMut(QmiResult<WdsPacketServiceStatus>)>;

use gobject_sys::GObject;

use std::ffi::c_void;

/// Packet data connection status reported by the modem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WdsPacketServiceStatus {
    /// Current connection status.
    pub status: WdsConnectionStatus,
    /// Whether the host must retrieve and apply updated network settings.
    pub reconfiguration_required: bool,
    /// IP family associated with the packet data session, when reported.
    pub ip_family: Option<WdsIpFamily>,
    /// General call termination reason, when reported.
    pub call_end_reason: Option<WdsCallEndReason>,
}

unsafe extern "C" fn packet_service_status_cb(
    _client: *mut GObject,
    output: *mut QmiIndicationWdsPacketServiceStatusOutput,
    user_data: *mut c_void,
) {
    tracing::debug!("WDS packet_service_status received");

    let cb = &mut *(user_data.cast::<PacketServiceStatusCallback>());

    // Mandatory TLV: contains both the connection status and the
    // reconfiguration-required flag.
    let mut status = 0;
    let mut reconfiguration_required = GFALSE;
    let mut error: *mut GError = null_mut();
    let ret = qmi_indication_wds_packet_service_status_output_get_connection_status(
        output,
        &mut status,
        &mut reconfiguration_required,
        &mut error,
    );
    if ret == GFALSE {
        cb(Err((error, "packet_service_status_cb").into()));
        return;
    }

    let mut value_ip_family = 0;
    let ret = qmi_indication_wds_packet_service_status_output_get_ip_family(
        output,
        &mut value_ip_family,
        null_mut(),
    );
    let ip_family = match ret {
        GFALSE => None,
        _ => Some(from_glib(value_ip_family)),
    };

    let mut value_call_end_reason = 0;
    let ret = qmi_indication_wds_packet_service_status_output_get_call_end_reason(
        output,
        &mut value_call_end_reason,
        null_mut(),
    );
    let call_end_reason = match ret {
        GFALSE => None,
        _ => Some(from_glib(value_call_end_reason)),
    };

    cb(Ok(WdsPacketServiceStatus {
        status: from_glib(status),
        reconfiguration_required: reconfiguration_required != GFALSE,
        ip_family,
        call_end_reason,
    }))
}

signal_only!(
    /// Registers a callback for packet data connection status changes.
    ///
    /// The callback is invoked whenever the modem reports that the packet
    /// data session was connected, disconnected, or reconfigured.
    signal_packet_service_status,
    PacketServiceStatusCallback,
    c"packet-service-status",
    QmiIndicationWdsPacketServiceStatusOutput,
    packet_service_status_cb,
    "signal_packet_service_status"
);
