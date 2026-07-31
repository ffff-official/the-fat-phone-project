pub use libqmi_glib_types_rs::{PdsOperatingMode, PdsTrackingSessionState};

// TODO : CREATE A REAL QMI LOC CLIENT instead of PDS
//
// TODO : Set Event Report + signal event-report
// TODO : Get/Set AGPS Config
// TODO : Reset

use super::*;

use crate::sys::*;

use glib_sys::{GFALSE, GTRUE, gboolean};
use libqmi_glib_types_rs::Service;
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

/// A client for interacting with the Position Determination Service (PDS) of a QMI device.
#[derive(Debug)]
pub struct PdsClient {
    client: NonNull<QmiClientPds>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(PdsClient);

client_clone_impl!(PdsClient);

impl PdsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(PdsClient, QmiClientPds, Service::Pds);

    close_client_impl!("pds");
}

allocate_client_callback!(allocate_client_cb, QmiClientPds, "allocate PDS client");

release_client_callback!(release_client_cb, "release PDS client");

type Context = PdsClient;

/// Current state of the GPS service.
///
/// This structure indicates whether the GPS service is enabled and reports
/// the current state of the positioning tracking session.
#[derive(Debug, Clone, Copy)]
pub struct PdsGpsServiceState {
    /// Whether the GPS service is currently enabled.
    pub enabled: bool,
    /// Current state of the GPS tracking session.
    pub tracking_session_state: PdsTrackingSessionState,
}

impl Default for PdsGpsServiceState {
    fn default() -> Self {
        Self {
            enabled: bool::default(),
            tracking_session_state: PdsTrackingSessionState::__Unknown(0),
        }
    }
}

unsafe fn get_gps_service_state_extractor(
    output: *mut QmiMessagePdsGetGpsServiceStateOutput,
    error: *mut *mut GError,
    param: &mut PdsGpsServiceState,
) -> gboolean {
    let mut my_bool = 0;
    let mut tracking_session_state = 0;
    let ret = qmi_message_pds_get_gps_service_state_output_get_state(
        output,
        &mut my_bool,
        &mut tracking_session_state,
        error,
    );
    if ret == GTRUE {
        *param = PdsGpsServiceState {
            enabled: my_bool != GFALSE,
            tracking_session_state: from_glib(tracking_session_state),
        };
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the current GPS service state.
    ///
    /// The returned information includes whether the GPS service is enabled
    /// and the current tracking session state.
    get_gps_service_state,
    get_gps_service_state_extractor,
    PdsGpsServiceState,
    qmi_client_pds_get_gps_service_state,
    qmi_client_pds_get_gps_service_state_finish,
    qmi_message_pds_get_gps_service_state_output_get_result,
    qmi_message_pds_get_gps_service_state_output_unref,
    "get_gps_service_state"
);

setter!(
    /// Enables or disables the GPS service.
    ///
    /// The supplied value determines whether the positioning service should
    /// be enabled on the device.
    set_gps_service_state,
    qmi_message_pds_set_gps_service_state_input_set_state,
    bool,
    qmi_message_pds_set_gps_service_state_input_new,
    qmi_message_pds_set_gps_service_state_input_unref,
    qmi_client_pds_set_gps_service_state,
    qmi_client_pds_set_gps_service_state_finish,
    qmi_message_pds_set_gps_service_state_output_get_result,
    qmi_message_pds_set_gps_service_state_output_unref,
    "set_gps_service_state"
);

getter!(
    /// Retrieves whether automatic GPS tracking is enabled.
    get_auto_tracking_state,
    qmi_message_pds_get_auto_tracking_state_output_get_state,
    bool,
    qmi_client_pds_get_auto_tracking_state,
    qmi_client_pds_get_auto_tracking_state_finish,
    qmi_message_pds_get_auto_tracking_state_output_get_result,
    qmi_message_pds_get_auto_tracking_state_output_unref,
    "get_auto_tracking_state"
);

setter!(
    /// Enables or disables automatic GPS tracking.
    set_auto_tracking_state,
    qmi_message_pds_set_auto_tracking_state_input_set_state,
    bool,
    qmi_message_pds_set_auto_tracking_state_input_new,
    qmi_message_pds_set_auto_tracking_state_input_unref,
    qmi_client_pds_set_auto_tracking_state,
    qmi_client_pds_set_auto_tracking_state_finish,
    qmi_message_pds_set_auto_tracking_state_output_get_result,
    qmi_message_pds_set_auto_tracking_state_output_unref,
    "set_auto_tracking_state"
);

/// Default configuration of a GPS tracking session.
///
/// This structure defines the parameters used when starting a positioning
/// tracking session without explicitly providing a custom configuration.
#[derive(Debug, Clone, Copy)]
pub struct PdsDefaultTrackingSession {
    /// Operating mode used for the tracking session.
    pub operating_mode: PdsOperatingMode,
    /// Maximum time, in seconds, to wait for a position fix.
    pub position_data_timeout: u8,
    /// Interval, in milliseconds, between successive position reports.
    pub interval: u32,
    /// Desired positioning accuracy threshold.
    ///
    /// The unit is modem-dependent and follows the value expected by the
    /// underlying QMI service.
    pub accuracy_threshold: u32,
}

impl Default for PdsDefaultTrackingSession {
    fn default() -> Self {
        Self {
            operating_mode: PdsOperatingMode::__Unknown(0),
            position_data_timeout: u8::default(),
            interval: u32::default(),
            accuracy_threshold: u32::default(),
        }
    }
}

#[cfg(feature = "v1_12")]
unsafe fn get_default_tracking_session_extractor(
    output: *mut QmiMessagePdsGetDefaultTrackingSessionOutput,
    error: *mut *mut GError,
    param: &mut PdsDefaultTrackingSession,
) -> gboolean {
    let mut operating_mode = 0;

    let ret = qmi_message_pds_get_default_tracking_session_output_get_info(
        output,
        &mut operating_mode,
        &mut param.position_data_timeout,
        &mut param.interval,
        &mut param.accuracy_threshold,
        error,
    );

    if ret == GTRUE {
        param.operating_mode = from_glib(operating_mode);
    }

    ret
}

getter_with_extractor!(
    /// Retrieves the default positioning tracking session configuration.
    ///
    /// The returned information includes the operating mode, position data
    /// timeout, tracking interval, and accuracy threshold.
    #[cfg(feature = "v1_12")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_12")))]
    get_default_tracking_session,
    get_default_tracking_session_extractor,
    PdsDefaultTrackingSession,
    qmi_client_pds_get_default_tracking_session,
    qmi_client_pds_get_default_tracking_session_finish,
    qmi_message_pds_get_default_tracking_session_output_get_result,
    qmi_message_pds_get_default_tracking_session_output_unref,
    "get_default_tracking_session"
);

#[cfg(feature = "v1_12")]
unsafe fn set_default_tracking_session_writer(
    input: *mut QmiMessagePdsSetDefaultTrackingSessionInput,
    error: *mut *mut GError,
    parameter: PdsDefaultTrackingSession,
) -> gboolean {
    qmi_message_pds_set_default_tracking_session_input_set_info(
        input,
        parameter.operating_mode.into_glib(),
        parameter.position_data_timeout,
        parameter.interval,
        parameter.accuracy_threshold,
        error,
    )
}

setter_with_writer!(
    /// Sets the default positioning tracking session configuration.
    ///
    /// The supplied configuration specifies the operating mode, position data
    /// timeout, tracking interval, and accuracy threshold.
    #[cfg(feature = "v1_12")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_12")))]
    set_default_tracking_session,
    set_default_tracking_session_writer,
    PdsDefaultTrackingSession,
    qmi_message_pds_set_default_tracking_session_input_new,
    qmi_message_pds_set_default_tracking_session_input_unref,
    qmi_client_pds_set_default_tracking_session,
    qmi_client_pds_set_default_tracking_session_finish,
    qmi_message_pds_set_default_tracking_session_output_get_result,
    qmi_message_pds_set_default_tracking_session_output_unref,
    "set_default_tracking_session"
);
