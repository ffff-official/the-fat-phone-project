pub use libqmi_glib_types_rs::{
    DmsDataServiceCapability, DmsOperatingMode, DmsPowerState, DmsRadioInterface, DmsSimCapability,
    DmsTimeSource,
};

use futures::channel::oneshot;
use gio_sys::GAsyncResult;
use glib_sys::{GArray, GFALSE, GTRUE, gboolean};
use gobject_sys::GObject;
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
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

macro_rules! getter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
        $return_type:ty,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl DmsClient {
            $(#[$meta])*
            pub async fn $method_name(&self) -> QmiResult<$return_type> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == GFALSE {
                        $message_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut flags = 0;
                    let ret = $qmi_function(
                        output,
                        &mut flags,
                        &mut error,
                    );
                    $message_output_unref(output);
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(from_glib(flags))
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        null_mut(),
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    };
}

macro_rules! setter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
        $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl DmsClient {
            $(#[$meta])*
            pub async fn $method_name(&mut self, parameter: $parameter_type) -> QmiResult<()> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    $message_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { $message_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = null_mut();
                let ret = unsafe { $qmi_function(input, parameter.into_glib(), &mut error) };
                if ret == GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

macro_rules! getter_with_extractor {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $extractor:path,
        $return_type:ty,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl DmsClient {
            $(#[$meta])*
            pub async fn $method_name(&self) -> QmiResult<$return_type> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<$return_type>>>());

                    let result = get_result(client, res);

                    drop(sender.send(result).inspect_err(cannot_send));
                }
                unsafe fn get_result(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                ) -> QmiResult<$return_type> {
                    let mut error: *mut GError = null_mut();

                    let output = $validation_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = $message_output_get_result(output, &mut error);
                    if ret == GFALSE {
                        $message_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut error: *mut GError = null_mut();
                    let mut final_result = <$return_type>::default();
                    let ret = $extractor(output, &mut error, &mut final_result);

                    $message_output_unref(output);
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(final_result)
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        null_mut(),
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    );
                }

                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    };
}

macro_rules! setter_with_writer {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $writer:path,
        $parameter_type:ty,
        $message_input_new:ident,
        $message_input_unref:ident,
        $validation_start:ident,
        $validation_finish:ident,
        $message_output_get_result:ident,
        $message_output_unref:ident,
        $context:literal
    ) => {
        impl DmsClient {
            $(#[$meta])*
            pub async fn $method_name(&mut self, parameter: $parameter_type) -> QmiResult<()> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = $validation_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = $message_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    $message_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { $message_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = null_mut();
                let ret = unsafe { $writer(input, &mut error, parameter) };
                if ret == GFALSE {
                    unsafe { $message_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                unsafe {
                    $validation_start(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { $message_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

setter!(
    // TODO : Documentation
    set_operating_mode,
    qmi_message_dms_set_operating_mode_input_set_mode,
    DmsOperatingMode,
    qmi_message_dms_set_operating_mode_input_new,
    qmi_message_dms_set_operating_mode_input_unref,
    qmi_client_dms_set_operating_mode,
    qmi_client_dms_set_operating_mode_finish,
    qmi_message_dms_set_operating_mode_output_get_result,
    qmi_message_dms_set_operating_mode_output_unref,
    "set_operating_mode"
);

getter!(
    // TODO : Documentation
    get_operating_mode,
    qmi_message_dms_get_operating_mode_output_get_mode,
    DmsOperatingMode,
    qmi_client_dms_get_operating_mode,
    qmi_client_dms_get_operating_mode_finish,
    qmi_message_dms_get_operating_mode_output_get_result,
    qmi_message_dms_get_operating_mode_output_unref,
    "get_operating_mode"
);

/// TODO : Documentation
#[derive(Debug, Clone)]
pub struct DmsCapabilities {
    pub max_tx_channel_rate: u32,
    pub max_rx_channel_rate: u32,
    pub data_service_capability: DmsDataServiceCapability,
    pub sim_capability: DmsSimCapability,
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
    // TODO : Documentation
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
    // TODO : Documentation
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
    // TODO : Documentation
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
    // TODO : Documentation
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
    // TODO : Documentation
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
    // TODO : Documentation
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
    // TODO : Documentation
    get_factory_sku,
    get_factory_sku_extractor,
    String,
    qmi_client_dms_get_factory_sku,
    qmi_client_dms_get_factory_sku_finish,
    qmi_message_dms_get_factory_sku_output_get_result,
    qmi_message_dms_get_factory_sku_output_unref,
    "get_factory_sku"
);

/// TODO : Documentation
#[derive(Debug, Clone, Default)]
pub struct DmsIds {
    pub esn: Option<String>,
    pub imei: Option<String>,
    pub meid: Option<String>,
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
    // TODO : Documentation
    get_ids,
    get_ids_extractor,
    DmsIds,
    qmi_client_dms_get_ids,
    qmi_client_dms_get_ids_finish,
    qmi_message_dms_get_ids_output_get_result,
    qmi_message_dms_get_ids_output_unref,
    "get_ids"
);

/// TODO : Documentation
#[derive(Debug, Clone)]
pub struct DmsPowerStateLevel {
    pub flags: DmsPowerState,
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
    // TODO : Documentation
    get_power_state,
    get_power_state_extractor,
    DmsPowerStateLevel,
    qmi_client_dms_get_power_state,
    qmi_client_dms_get_power_state_finish,
    qmi_message_dms_get_power_state_output_get_result,
    qmi_message_dms_get_power_state_output_unref,
    "get_power_state"
);

/// TODO : Documentation
#[derive(Debug, Clone, Default)]
pub struct PrlVersion {
    pub version: u16,
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
    // TODO : Documentation
    get_prl_version,
    get_prl_version_extractor,
    PrlVersion,
    qmi_client_dms_get_prl_version,
    qmi_client_dms_get_prl_version_finish,
    qmi_message_dms_get_prl_version_output_get_result,
    qmi_message_dms_get_prl_version_output_unref,
    "get_prl_version"
);

/// TODO : Documentation
#[derive(Debug, Clone)]
pub struct DmsTime {
    pub time_source: DmsTimeSource,
    pub device_time: u64,
    pub system_time: u64,
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
    // TODO : Documentation
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
    // TODO : Documentation
    get_user_lock_state,
    qmi_message_dms_get_user_lock_state_output_get_enabled,
    bool,
    qmi_client_dms_get_user_lock_state,
    qmi_client_dms_get_user_lock_state_finish,
    qmi_message_dms_get_user_lock_state_output_get_result,
    qmi_message_dms_get_user_lock_state_output_unref,
    "get_user_lock_state"
);

/// TODO : Documeentation
#[derive(Debug, Clone)]
pub struct DmsUserLockState {
    pub enabled: bool,
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
    // TODO : Documentation
    set_user_lock_state,
    set_user_lock_state_writer,
    DmsUserLockState,
    qmi_message_dms_set_user_lock_state_input_new,
    qmi_message_dms_set_user_lock_state_input_unref,
    qmi_client_dms_set_user_lock_state,
    qmi_client_dms_set_user_lock_state_finish,
    qmi_message_dms_set_user_lock_state_output_get_result,
    qmi_message_dms_set_user_lock_state_output_unref,
    "set_user_lock_state"
);

unsafe fn get_supported_messages_extractor(
    output: *mut QmiMessageDmsGetSupportedMessagesOutput,
    error: *mut *mut GError,
    param: &mut Vec<u8>,
) -> gboolean {
    let mut value_list: *mut GArray = null_mut();
    let ret =
        qmi_message_dms_get_supported_messages_output_get_list(output, &mut value_list, error);
    if ret == GTRUE && !value_list.is_null() {
        let slice =
            std::slice::from_raw_parts((*value_list).data.cast::<u8>(), (*value_list).len as usize);
        *param = slice.iter().cloned().collect();
    }
    ret
}

getter_with_extractor!(
    // TODO : Documentation
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
    // TODO : Documentation
    get_msisdn,
    get_msisdn_extractor,
    String,
    qmi_client_dms_get_msisdn,
    qmi_client_dms_get_msisdn_finish,
    qmi_message_dms_get_msisdn_output_get_result,
    qmi_message_dms_get_msisdn_output_unref,
    "get_msisdn"
);
