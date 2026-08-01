pub use libqmi_glib_types_rs::{
    DmsDataServiceCapability, DmsOperatingMode, DmsPowerState, DmsRadioInterface, DmsSimCapability,
    DmsTimeSource,
};

use glib_sys::{GArray, GFALSE, GTRUE, gboolean};
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ffi::{CString, c_char};
use std::ptr::{NonNull, null, null_mut};
use std::rc::Rc;

/// A client for interacting with the Device Management Service (DMS) of a QMI device.
#[derive(Debug)]
pub struct DmsClient {
    client: NonNull<QmiClientDms>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
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
}

allocate_client_callback!(allocate_client_cb, QmiClientDms, "allocate DMS client");

release_client_callback!(release_client_cb, "release DMS client");

type Context = DmsClient;

setter!(
    /// Sets the operating mode of the device.
    ///
    /// The requested operating mode determines the operational state of the
    /// modem, such as online, low-power, offline, or reset mode.
    set_operating_mode,
    qmi_message_dms_set_operating_mode_input_set_mode,
    mode: DmsOperatingMode,
    qmi_message_dms_set_operating_mode_input_new,
    qmi_message_dms_set_operating_mode_input_unref,
    qmi_client_dms_set_operating_mode,
    qmi_client_dms_set_operating_mode_finish,
    qmi_message_dms_set_operating_mode_output_get_result,
    qmi_message_dms_set_operating_mode_output_unref,
    "set_operating_mode"
);

getter!(
    /// Retrieves the current operating mode of the device.
    ///
    /// The operating mode indicates the current operational state of the modem,
    /// such as online, low-power, offline, or reset mode.
    get_operating_mode,
    qmi_message_dms_get_operating_mode_output_get_mode,
    DmsOperatingMode,
    qmi_client_dms_get_operating_mode,
    qmi_client_dms_get_operating_mode_finish,
    qmi_message_dms_get_operating_mode_output_get_result,
    qmi_message_dms_get_operating_mode_output_unref,
    "get_operating_mode"
);

/// Describes the capabilities reported by the device.
#[derive(Debug, Clone)]
pub struct DmsCapabilities {
    /// Maximum transmit channel rate supported by the device, in bits per second.
    pub max_tx_channel_rate: u32,
    /// Maximum receive channel rate supported by the device, in bits per second.
    pub max_rx_channel_rate: u32,
    /// Data service capability reported by the device.
    pub data_service_capability: DmsDataServiceCapability,
    /// SIM capability reported by the device.
    pub sim_capability: DmsSimCapability,
    /// Radio interfaces supported by the device.
    pub radio_interfaces: Vec<DmsRadioInterface>,
}

impl Default for DmsCapabilities {
    fn default() -> Self {
        Self {
            max_tx_channel_rate: u32::default(),
            max_rx_channel_rate: u32::default(),
            data_service_capability: DmsDataServiceCapability::__Unknown(0),
            sim_capability: DmsSimCapability::__Unknown(0),
            radio_interfaces: Vec::new(),
        }
    }
}

unsafe fn get_capabilities_extractor(
    output: *mut QmiMessageDmsGetCapabilitiesOutput,
    error: *mut *mut GError,
    param: &mut DmsCapabilities,
) -> gboolean {
    let mut data_service_capability = 0;
    let mut sim_capability = 0;
    let mut value_info_radio_interface_list: *mut GArray = null_mut();
    let ret = qmi_message_dms_get_capabilities_output_get_info(
        output,
        &mut param.max_tx_channel_rate,
        &mut param.max_rx_channel_rate,
        &mut data_service_capability,
        &mut sim_capability,
        &mut value_info_radio_interface_list,
        error,
    );
    if ret == GTRUE {
        param.data_service_capability = from_glib(data_service_capability);
        param.sim_capability = from_glib(sim_capability);
        if !value_info_radio_interface_list.is_null() && (*value_info_radio_interface_list).len > 0
        {
            let slice = std::slice::from_raw_parts(
                (*value_info_radio_interface_list)
                    .data
                    .cast::<QmiDmsRadioInterface>(),
                (*value_info_radio_interface_list).len as usize,
            );
            param.radio_interfaces = slice.iter().map(|iface| from_glib(*iface)).collect();
        }
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the capabilities reported by the device.
    ///
    /// The returned information includes the maximum transmit and receive
    /// channel rates, data service and SIM capabilities, and the list of
    /// radio interfaces supported by the modem.
    get_capabilities,
    get_capabilities_extractor,
    DmsCapabilities,
    qmi_client_dms_get_capabilities,
    qmi_client_dms_get_capabilities_finish,
    qmi_message_dms_get_capabilities_output_get_result,
    qmi_message_dms_get_capabilities_output_unref,
    "get_capabilities"
);
unsafe fn get_manufacturer_extractor(
    output: *mut QmiMessageDmsGetManufacturerOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_manufacturer_output_get_manufacturer(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the manufacturer name reported by the device.
    ///
    /// The returned string identifies the manufacturer of the modem.
    get_manufacturer,
    get_manufacturer_extractor,
    String,
    qmi_client_dms_get_manufacturer,
    qmi_client_dms_get_manufacturer_finish,
    qmi_message_dms_get_manufacturer_output_get_result,
    qmi_message_dms_get_manufacturer_output_unref,
    "get_manufacturer"
);

unsafe fn get_model_extractor(
    output: *mut QmiMessageDmsGetModelOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_model_output_get_model(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the model identifier reported by the device.
    ///
    /// The returned string identifies the modem model as reported by the device.
    get_model,
    get_model_extractor,
    String,
    qmi_client_dms_get_model,
    qmi_client_dms_get_model_finish,
    qmi_message_dms_get_model_output_get_result,
    qmi_message_dms_get_model_output_unref,
    "get_model"
);

unsafe fn get_revision_extractor(
    output: *mut QmiMessageDmsGetRevisionOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_revision_output_get_revision(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the firmware revision reported by the device.
    ///
    /// The returned string identifies the revision of the firmware currently
    /// running on the modem.
    get_revision,
    get_revision_extractor,
    String,
    qmi_client_dms_get_revision,
    qmi_client_dms_get_revision_finish,
    qmi_message_dms_get_revision_output_get_result,
    qmi_message_dms_get_revision_output_unref,
    "get_revision"
);

unsafe fn get_hardware_revision_extractor(
    output: *mut QmiMessageDmsGetHardwareRevisionOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_hardware_revision_output_get_revision(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the hardware revision reported by the device.
    ///
    /// The returned string identifies the hardware revision of the modem.
    get_hardware_revision,
    get_hardware_revision_extractor,
    String,
    qmi_client_dms_get_hardware_revision,
    qmi_client_dms_get_hardware_revision_finish,
    qmi_message_dms_get_hardware_revision_output_get_result,
    qmi_message_dms_get_hardware_revision_output_unref,
    "get_hardware_revision"
);

unsafe fn get_software_version_extractor(
    output: *mut QmiMessageDmsGetSoftwareVersionOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_software_version_output_get_version(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the software version reported by the device.
    ///
    /// The returned string identifies the software version of the modem.
    get_software_version,
    get_software_version_extractor,
    String,
    qmi_client_dms_get_software_version,
    qmi_client_dms_get_software_version_finish,
    qmi_message_dms_get_software_version_output_get_result,
    qmi_message_dms_get_software_version_output_unref,
    "get_software_version"
);

unsafe fn get_factory_sku_extractor(
    output: *mut QmiMessageDmsGetFactorySkuOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value: *const c_char = null();

    let ret = qmi_message_dms_get_factory_sku_output_get_sku(output, &mut value, error);

    if ret == GTRUE && !value.is_null() {
        *param = c_char_to_string(value);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the factory SKU reported by the device.
    ///
    /// The returned string identifies the device's factory stock keeping unit
    /// (SKU), when provided by the modem.
    get_factory_sku,
    get_factory_sku_extractor,
    String,
    qmi_client_dms_get_factory_sku,
    qmi_client_dms_get_factory_sku_finish,
    qmi_message_dms_get_factory_sku_output_get_result,
    qmi_message_dms_get_factory_sku_output_unref,
    "get_factory_sku"
);

/// Device identifiers reported by the modem.
///
/// Depending on the device and supported radio technologies, some identifiers
/// may not be available.
#[derive(Debug, Clone, Default)]
pub struct DmsIds {
    /// Electronic Serial Number (ESN), if available.
    pub esn: Option<String>,
    /// International Mobile Equipment Identity (IMEI), if available.
    pub imei: Option<String>,
    /// Mobile Equipment Identifier (MEID), if available.
    pub meid: Option<String>,
    /// IMEI software version, if available.
    pub imei_software_version: Option<String>,
}

unsafe fn get_ids_extractor(
    output: *mut QmiMessageDmsGetIdsOutput,
    _error: *mut *mut GError,
    param: &mut DmsIds,
) -> gboolean {
    let mut esn: *const c_char = null();
    let mut imei: *const c_char = null();
    let mut meid: *const c_char = null();
    let mut imei_software_version: *const c_char = null();

    let ret = qmi_message_dms_get_ids_output_get_esn(output, &mut esn, null_mut());
    if ret == GTRUE && !esn.is_null() {
        param.esn = Some(c_char_to_string(esn));
    }
    let ret = qmi_message_dms_get_ids_output_get_imei(output, &mut imei, null_mut());
    if ret == GTRUE && !imei.is_null() {
        param.imei = Some(c_char_to_string(imei));
    }
    let ret = qmi_message_dms_get_ids_output_get_meid(output, &mut meid, null_mut());
    if ret == GTRUE && !meid.is_null() {
        param.meid = Some(c_char_to_string(meid));
    }
    let ret = qmi_message_dms_get_ids_output_get_imei_software_version(
        output,
        &mut imei_software_version,
        null_mut(),
    );
    if ret == GTRUE && !imei_software_version.is_null() {
        param.imei_software_version = Some(c_char_to_string(imei_software_version));
    }
    GTRUE
}

getter_with_extractor!(
    /// Retrieves the device identifiers reported by the modem.
    ///
    /// Depending on the device and supported radio technologies, some
    /// identifiers may not be available.
    get_ids,
    get_ids_extractor,
    DmsIds,
    qmi_client_dms_get_ids,
    qmi_client_dms_get_ids_finish,
    qmi_message_dms_get_ids_output_get_result,
    qmi_message_dms_get_ids_output_unref,
    "get_ids"
);

/// Power state information reported by the device.
#[derive(Debug, Clone)]
pub struct DmsPowerStateLevel {
    /// Flags describing the current power state of the device.
    pub flags: DmsPowerState,
    /// Current battery level reported by the device, as a percentage.
    pub battery_level: u8,
}

impl Default for DmsPowerStateLevel {
    fn default() -> Self {
        Self {
            flags: DmsPowerState::empty(),
            battery_level: u8::default(),
        }
    }
}

unsafe fn get_power_state_extractor(
    output: *mut QmiMessageDmsGetPowerStateOutput,
    error: *mut *mut GError,
    param: &mut DmsPowerStateLevel,
) -> gboolean {
    let mut flags: u8 = 0;
    let ret = qmi_message_dms_get_power_state_output_get_info(
        output,
        &mut flags,
        &mut param.battery_level,
        error,
    );
    if ret == GTRUE {
        param.flags = from_glib(flags as u32);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the current power state information reported by the device.
    ///
    /// The returned information includes the device power state flags and
    /// the current battery level.
    get_power_state,
    get_power_state_extractor,
    DmsPowerStateLevel,
    qmi_client_dms_get_power_state,
    qmi_client_dms_get_power_state_finish,
    qmi_message_dms_get_power_state_output_get_result,
    qmi_message_dms_get_power_state_output_unref,
    "get_power_state"
);

/// Preferred Roaming List (PRL) version information reported by the device.
#[derive(Debug, Clone, Default)]
pub struct PrlVersion {
    /// Version number of the Preferred Roaming List (PRL).
    pub version: u16,
    /// Whether the device is configured to use only systems defined in the PRL.
    pub prl_only_preference: bool,
}

unsafe fn get_prl_version_extractor(
    output: *mut QmiMessageDmsGetPrlVersionOutput,
    _error: *mut *mut GError,
    param: &mut PrlVersion,
) -> gboolean {
    let _ret =
        qmi_message_dms_get_prl_version_output_get_version(output, &mut param.version, null_mut());
    let mut my_bool = 0;
    let ret = qmi_message_dms_get_prl_version_output_get_prl_only_preference(
        output,
        &mut my_bool,
        null_mut(),
    );
    if ret == GTRUE {
        param.prl_only_preference = my_bool != GFALSE;
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the Preferred Roaming List (PRL) version information.
    ///
    /// The returned information includes the PRL version and whether the
    /// device is configured to use only systems defined in the PRL.
    get_prl_version,
    get_prl_version_extractor,
    PrlVersion,
    qmi_client_dms_get_prl_version,
    qmi_client_dms_get_prl_version_finish,
    qmi_message_dms_get_prl_version_output_get_result,
    qmi_message_dms_get_prl_version_output_unref,
    "get_prl_version"
);

/// Time information reported by the device.
#[derive(Debug, Clone)]
pub struct DmsTime {
    /// Source used by the device to obtain its time information.
    pub time_source: DmsTimeSource,
    /// Device time, expressed as the number of 1.25 ms time units elapsed
    /// since January 6, 1980.
    pub device_time: u64,
    /// System time, expressed as the number of milliseconds elapsed
    /// since January 6, 1980.
    pub system_time: u64,
    /// User time, expressed as the number of milliseconds elapsed
    /// since January 6, 1980.
    pub user_time: u64,
}

impl Default for DmsTime {
    fn default() -> Self {
        Self {
            time_source: DmsTimeSource::__Unknown(0),
            device_time: u64::default(),
            system_time: u64::default(),
            user_time: u64::default(),
        }
    }
}

unsafe fn get_time_extractor(
    output: *mut QmiMessageDmsGetTimeOutput,
    _error: *mut *mut GError,
    param: &mut DmsTime,
) -> gboolean {
    let mut dms_time_source = 0;
    let ret = qmi_message_dms_get_time_output_get_device_time(
        output,
        &mut param.device_time,
        &mut dms_time_source,
        null_mut(),
    );
    if ret == GTRUE {
        param.time_source = from_glib(dms_time_source);
    }

    qmi_message_dms_get_time_output_get_system_time(output, &mut param.system_time, null_mut());

    qmi_message_dms_get_time_output_get_user_time(output, &mut param.user_time, null_mut());

    GTRUE
}

getter_with_extractor!(
    /// Retrieves the current time information reported by the device.
    ///
    /// The returned information includes the device, system, and user time,
    /// as well as the source used to obtain the time information.
    get_time,
    get_time_extractor,
    DmsTime,
    qmi_client_dms_get_time,
    qmi_client_dms_get_time_finish,
    qmi_message_dms_get_time_output_get_result,
    qmi_message_dms_get_time_output_unref,
    "get_time"
);

getter!(
    /// Retrieves the current user lock state of the device.
    ///
    /// Returns `true` if the user lock is enabled, or `false` if it is disabled.
    get_user_lock_state,
    qmi_message_dms_get_user_lock_state_output_get_enabled,
    bool,
    qmi_client_dms_get_user_lock_state,
    qmi_client_dms_get_user_lock_state_finish,
    qmi_message_dms_get_user_lock_state_output_get_result,
    qmi_message_dms_get_user_lock_state_output_unref,
    "get_user_lock_state"
);

/// Configuration used to enable or disable the device user lock.
///
/// The current user lock code is required to authorize the requested change.
#[derive(Debug, Clone)]
pub struct DmsUserLockState {
    /// Whether the user lock should be enabled.
    pub enabled: bool,
    /// Current user lock code used to authorize the operation.
    pub code: String,
}

unsafe fn set_user_lock_state_writer(
    input: *mut QmiMessageDmsSetUserLockStateInput,
    error: *mut *mut GError,
    parameter: DmsUserLockState,
) -> gboolean {
    match CString::new(parameter.code) {
        Ok(code) => qmi_message_dms_set_user_lock_state_input_set_info(
            input,
            parameter.enabled.into_glib(),
            code.as_ptr(),
            error,
        ),
        Err(e) => {
            tracing::error!("CRITICAL : Cannot set CString : {:?}", e);
            GTRUE
        }
    }
}

setter_with_writer!(
    /// Enables or disables the device user lock.
    ///
    /// The current user lock code must be provided to authorize the operation.
    set_user_lock_state,
    set_user_lock_state_writer,
    state: DmsUserLockState,
    qmi_message_dms_set_user_lock_state_input_new,
    qmi_message_dms_set_user_lock_state_input_unref,
    qmi_client_dms_set_user_lock_state,
    qmi_client_dms_set_user_lock_state_finish,
    qmi_message_dms_set_user_lock_state_output_get_result,
    qmi_message_dms_set_user_lock_state_output_unref,
    "set_user_lock_state"
);

#[cfg(feature = "v1_14")]
unsafe fn get_supported_messages_extractor(
    output: *mut QmiMessageDmsGetSupportedMessagesOutput,
    error: *mut *mut GError,
    param: &mut Vec<u8>,
) -> gboolean {
    let mut value_list: *mut GArray = null_mut();
    let ret =
        qmi_message_dms_get_supported_messages_output_get_list(output, &mut value_list, error);
    if ret == GTRUE && !value_list.is_null() && (*value_list).len > 0 {
        let slice =
            std::slice::from_raw_parts((*value_list).data.cast::<u8>(), (*value_list).len as usize);
        *param = slice.to_vec();
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the list of DMS messages supported by the device.
    ///
    /// The returned byte vector contains a bitmap indicating which DMS
    /// message identifiers are supported by the modem.
    #[cfg(feature = "v1_14")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
    get_supported_messages,
    get_supported_messages_extractor,
    Vec<u8>,
    qmi_client_dms_get_supported_messages,
    qmi_client_dms_get_supported_messages_finish,
    qmi_message_dms_get_supported_messages_output_get_result,
    qmi_message_dms_get_supported_messages_output_unref,
    "get_supported_messages"
);
unsafe fn get_msisdn_extractor(
    output: *mut QmiMessageDmsGetMsisdnOutput,
    error: *mut *mut GError,
    param: &mut String,
) -> gboolean {
    let mut value_msisdn: *const c_char = null();
    let ret = qmi_message_dms_get_msisdn_output_get_msisdn(output, &mut value_msisdn, error);
    if ret == GTRUE && !value_msisdn.is_null() {
        *param = c_char_to_string(value_msisdn);
    }
    ret
}

getter_with_extractor!(
    /// Retrieves the Mobile Station ISDN Number (MSISDN) reported by the device.
    ///
    /// The returned string contains the telephone number associated with the
    /// subscription, when available.
    get_msisdn,
    get_msisdn_extractor,
    String,
    qmi_client_dms_get_msisdn,
    qmi_client_dms_get_msisdn_finish,
    qmi_message_dms_get_msisdn_output_get_result,
    qmi_message_dms_get_msisdn_output_unref,
    "get_msisdn"
);
