mod event_report;
mod signal_info;
pub use event_report::NasEventReport;
pub use libqmi_glib_types_rs::{
    NasBandPreference, NasEvdoSinrLevel, NasGsmWcdmaAcquisitionOrderPreference,
    NasLteBandPreference, NasNetworkSelectionPreference, NasRadioInterface, NasRatModePreference,
    NasRoamingPreference, NasServiceDomainPreference, NasUsagePreference, NasVoiceDomainPreference,
};
pub use signal_info::SignalInfo;

use futures::channel::oneshot::{self, Sender};
use gio_sys::GAsyncResult;
use glib_sys::{GArray, GFALSE, GTRUE, g_array_unref, gboolean};
use gobject_sys::GObject;
use libqmi_glib_types_rs::DmsRadioInterface;
use libqmi_glib_types_rs::glib::translate::{IntoGlib, from_glib};

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

client_drop_impl!(NasClient);

client_clone_impl!(NasClient);

impl NasClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(NasClient, QmiClientNas, libqmi_glib_types_rs::Service::Nas);

    close_client_impl!("nas");
}

allocate_client_callback!(allocate_client_cb, QmiClientNas, "allocate NAS client");

release_client_callback!(release_client_cb, "release NAS client");

macro_rules! getter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
        $return_type:ty,
        $context:literal
    ) => {
        impl NasClient {
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

                    let output = qmi_client_nas_get_system_selection_preference_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = qmi_message_nas_get_system_selection_preference_output_get_result(output, &mut error);
                    if ret == GFALSE {
                        qmi_message_nas_get_system_selection_preference_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut flags = 0;
                    let ret = $qmi_function(
                        output,
                        &mut flags,
                        &mut error,
                    );
                    qmi_message_nas_get_system_selection_preference_output_unref(output);
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(from_glib(flags))
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    qmi_client_nas_get_system_selection_preference(
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
        $context:literal
        $(, $extra_parameter:ident : $extra_type:ty)*
    ) => {
        impl NasClient {
            $(#[$meta])*
            pub async fn $method_name(&mut self, parameter: $parameter_type, $($extra_parameter: $extra_type),*) -> QmiResult<()> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = qmi_client_nas_set_system_selection_preference_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = qmi_message_nas_set_system_selection_preference_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    qmi_message_nas_set_system_selection_preference_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { qmi_message_nas_set_system_selection_preference_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = null_mut();
                let ret = unsafe { $qmi_function(input, parameter.into_glib(), $($extra_parameter,)* &mut error) };
                if ret == GFALSE {
                    unsafe { qmi_message_nas_set_system_selection_preference_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                unsafe {
                    qmi_client_nas_set_system_selection_preference(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { qmi_message_nas_set_system_selection_preference_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

macro_rules! getter_with_custom_extractor {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $extractor:path,
        $return_type:ty,
        $context:literal
    ) => {
        impl NasClient {
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

                    let output = qmi_client_nas_get_system_selection_preference_finish(client.cast(), res, &mut error);
                    if output.is_null() {
                        return Err((error, concat!($context, " finish")).into());
                    }

                    let ret = qmi_message_nas_get_system_selection_preference_output_get_result(output, &mut error);
                    if ret == GFALSE {
                        qmi_message_nas_get_system_selection_preference_output_unref(output);
                        return Err((error, concat!($context, " get result")).into());
                    }

                    let mut error: *mut GError = null_mut();
                    let mut final_result = <$return_type>::default();
                    let ret = $extractor(output, &mut error, &mut final_result);

                    qmi_message_nas_get_system_selection_preference_output_unref(output);
                    if ret == GFALSE {
                        return Err((error, concat!($context, " exec qmi function")).into());
                    }
                    Ok(final_result)
                }
                let (tx, rx) = oneshot::channel::<QmiResult<$return_type>>();

                unsafe {
                    qmi_client_nas_get_system_selection_preference(
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
        $context:literal
    ) => {
        impl NasClient {
            $(#[$meta])*
            pub async fn $method_name(&mut self, parameter: $parameter_type) -> QmiResult<()> {
                unsafe extern "C" fn callback(
                    client: *mut GObject,
                    res: *mut GAsyncResult,
                    user_data: *mut c_void,
                ) {
                    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());

                    let mut error: *mut GError = null_mut();

                    let output_ptr = qmi_client_nas_set_system_selection_preference_finish(client.cast(), res, &mut error);

                    if output_ptr.is_null() {
                        let _r = sender
                            .send(Err((error, concat!($context, " cb finish")).into()))
                            .inspect_err(cannot_send);
                        return;
                    }

                    let ret = qmi_message_nas_set_system_selection_preference_output_get_result(output_ptr, &mut error);
                    let output = match ret {
                        GFALSE => Err((error, concat!($context, " cb get_result")).into()),
                        _ => Ok(()),
                    };
                    qmi_message_nas_set_system_selection_preference_output_unref(output_ptr);
                    let _r = sender.send(output).inspect_err(cannot_send);
                }
                let input = unsafe { qmi_message_nas_set_system_selection_preference_input_new() };

                if input.is_null() {
                    Err(QmiError::new("Null pointer", $context))?;
                }

                let mut error: *mut GError = null_mut();
                let ret = unsafe { $writer(input, &mut error, parameter) };
                if ret == GFALSE {
                    unsafe { qmi_message_nas_set_system_selection_preference_input_unref(input) };
                    Err((error, concat!($context, " assign")))?;
                }

                let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                unsafe {
                    qmi_client_nas_set_system_selection_preference(
                        self.client.as_ptr(),
                        input,
                        OPERATION_TIMEOUT,
                        null_mut(),
                        Some(callback),
                        Box::into_raw(Box::new(tx)).cast(),
                    )
                };

                unsafe { qmi_message_nas_set_system_selection_preference_input_unref(input) };
                rx.await.map_err(|_| QmiError::cb_droped())?
            }
        }
    }
}

getter!(
    /// TODO : Documentation
    get_preferred_radio_technologies,
    qmi_message_nas_get_system_selection_preference_output_get_mode_preference,
    NasRatModePreference,
    "get_preferred_radio_technologies"
);

setter!(
    /// TODO : Documentation
    set_preferred_radio_technologies,
    qmi_message_nas_set_system_selection_preference_input_set_mode_preference,
    NasRatModePreference,
    "set_preferred_radio_technologies"
);

getter!(
    /// TODO : Documentation
    get_network_selection_mode,
    qmi_message_nas_get_system_selection_preference_output_get_network_selection_preference,
    NasNetworkSelectionPreference,
    "get_network_selection_mode"
);

setter!(
    /// TODO : Documentation
    set_network_selection_mode,
    qmi_message_nas_set_system_selection_preference_input_set_network_selection_preference,
    NasNetworkSelectionPreference,
    "set_network_selection_mode",
    mcc: u16,
    mnc: u16
);

getter!(
    /// TODO : Documentation
    get_roaming_preference,
    qmi_message_nas_get_system_selection_preference_output_get_roaming_preference,
    NasRoamingPreference,
    "get_roaming_preference"
);

setter!(
    /// TODO : Documentation
    set_roaming_preference,
    qmi_message_nas_set_system_selection_preference_input_set_roaming_preference,
    NasRoamingPreference,
    "set_roaming_preference"
);

getter!(
    /// TODO : Documentation
    get_service_domain_preference,
    qmi_message_nas_get_system_selection_preference_output_get_service_domain_preference,
    NasServiceDomainPreference,
    "get_service_domain_preference"
);

setter!(
    /// TODO : Documentation
    set_service_domain_preference,
    qmi_message_nas_set_system_selection_preference_input_set_service_domain_preference,
    NasServiceDomainPreference,
    "set_service_domain_preference"
);

getter!(
    /// TODO : Documentation
    get_gsm_umts_acquisition_preference,
    qmi_message_nas_get_system_selection_preference_output_get_gsm_wcdma_acquisition_order_preference,
    NasGsmWcdmaAcquisitionOrderPreference,
    "get_gsm_umts_acquisition_preference"
);

setter!(
    /// TODO : Documentation
    set_gsm_umts_acquisition_preference,
    qmi_message_nas_set_system_selection_preference_input_set_gsm_wcdma_acquisition_order_preference,
    NasGsmWcdmaAcquisitionOrderPreference,
    "set_gsm_umts_acquisition_preference"
);

getter!(
    /// TODO : Documentation
    get_band_preference,
    qmi_message_nas_get_system_selection_preference_output_get_band_preference,
    NasBandPreference,
    "get_band_preference"
);

setter!(
    /// TODO : Documentation
    set_band_preference,
    qmi_message_nas_set_system_selection_preference_input_set_band_preference,
    NasBandPreference,
    "set_band_preference"
);

getter!(
    /// TODO : Documentation
    get_lte_band_preference,
    qmi_message_nas_get_system_selection_preference_output_get_lte_band_preference,
    NasLteBandPreference,
    "get_lte_band_preference"
);

setter!(
    /// TODO : Documentation
    set_lte_band_preference,
    qmi_message_nas_set_system_selection_preference_input_set_lte_band_preference,
    NasLteBandPreference,
    "set_lte_band_preference"
);

getter!(
    /// TODO : Documentation
    get_emergency_mode,
    qmi_message_nas_get_system_selection_preference_output_get_emergency_mode,
    bool,
    "get_emergency_mode"
);

setter!(
    /// TODO : Documentation
    set_emergency_mode,
    qmi_message_nas_set_system_selection_preference_input_set_emergency_mode,
    bool,
    "set_emergency_mode"
);

getter!(
    /// TODO : Documentation
    set_usage_preference,
    qmi_message_nas_get_system_selection_preference_output_get_usage_preference,
    NasUsagePreference,
    "set_usage_preference"
);

setter!(
    /// TODO : Documentation
    get_usage_preference,
    qmi_message_nas_set_system_selection_preference_input_set_usage_preference,
    NasUsagePreference,
    "get_usage_preference"
);

getter!(
    /// TODO : Documentation
    get_voice_domain_preference,
    qmi_message_nas_get_system_selection_preference_output_get_voice_domain_preference,
    NasVoiceDomainPreference,
    "get_voice_domain_preference"
);

setter!(
    /// TODO : Documentation
    set_voice_domain_preference,
    qmi_message_nas_set_system_selection_preference_input_set_voice_domain_preference,
    NasVoiceDomainPreference,
    "set_voice_domain_preference"
);

getter!(
    /// TODO : Documentation
    get_disabled_radio_technologies,
    qmi_message_nas_get_system_selection_preference_output_get_disabled_modes,
    NasRatModePreference,
    "get_disabled_radio_technologies"
);

/// TODO : Documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Nr5gSaBandPreference {
    /// TODO : Documentation
    pub masks: [u64; 8],
}

unsafe fn get_nr5g_sa_band_preference_extractor(
    output: *mut QmiMessageNasGetSystemSelectionPreferenceOutput,
    error: *mut *mut GError,
    param: &mut Nr5gSaBandPreference,
) -> gboolean {
    qmi_message_nas_get_system_selection_preference_output_get_nr5g_sa_band_preference(
        output,
        &mut param.masks[0],
        &mut param.masks[1],
        &mut param.masks[2],
        &mut param.masks[3],
        &mut param.masks[4],
        &mut param.masks[5],
        &mut param.masks[6],
        &mut param.masks[7],
        error,
    )
}

unsafe fn set_nr5g_sa_band_preference_writer(
    input: *mut QmiMessageNasSetSystemSelectionPreferenceInput,
    error: *mut *mut GError,
    param: Nr5gSaBandPreference,
) -> gboolean {
    qmi_message_nas_set_system_selection_preference_input_set_nr5g_sa_band_preference(
        input,
        param.masks[0],
        param.masks[1],
        param.masks[2],
        param.masks[3],
        param.masks[4],
        param.masks[5],
        param.masks[6],
        param.masks[7],
        error,
    )
}

getter_with_custom_extractor!(
    /// TODO : Documentation
    get_nr5g_sa_band_preference,
    get_nr5g_sa_band_preference_extractor,
    Nr5gSaBandPreference,
    "get_nr5g_sa_band_preference"
);

setter_with_writer!(
    /// TODO : Documentation
    set_nr5g_sa_band_preference,
    set_nr5g_sa_band_preference_writer,
    Nr5gSaBandPreference,
    "set_nr5g_sa_band_preference"
);

/// TODO : Documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Nr5gNsaBandPreference {
    /// TODO : Documentation
    pub masks: [u64; 8],
}

unsafe fn get_nr5g_nsa_band_preference_extractor(
    output: *mut QmiMessageNasGetSystemSelectionPreferenceOutput,
    error: *mut *mut GError,
    param: &mut Nr5gNsaBandPreference,
) -> gboolean {
    qmi_message_nas_get_system_selection_preference_output_get_nr5g_nsa_band_preference(
        output,
        &mut param.masks[0],
        &mut param.masks[1],
        &mut param.masks[2],
        &mut param.masks[3],
        &mut param.masks[4],
        &mut param.masks[5],
        &mut param.masks[6],
        &mut param.masks[7],
        error,
    )
}

unsafe fn set_nr5g_nsa_band_preference_writer(
    input: *mut QmiMessageNasSetSystemSelectionPreferenceInput,
    error: *mut *mut GError,
    param: Nr5gNsaBandPreference,
) -> gboolean {
    qmi_message_nas_set_system_selection_preference_input_set_nr5g_nsa_band_preference(
        input,
        param.masks[0],
        param.masks[1],
        param.masks[2],
        param.masks[3],
        param.masks[4],
        param.masks[5],
        param.masks[6],
        param.masks[7],
        error,
    )
}

getter_with_custom_extractor!(
    /// TODO : Documentation
    get_nr5g_nsa_band_preference,
    get_nr5g_nsa_band_preference_extractor,
    Nr5gNsaBandPreference,
    "get_nr5g_nsa_band_preference"
);

setter_with_writer!(
    /// TODO : Documentation
    set_nr5g_nsa_band_preference,
    set_nr5g_nsa_band_preference_writer,
    Nr5gNsaBandPreference,
    "set_nr5g_nsa_band_preference"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ExtendedLteBandPreference {
    pub masks: [u64; 4],
}

unsafe fn get_extended_lte_band_preference_extractor(
    output: *mut QmiMessageNasGetSystemSelectionPreferenceOutput,
    error: *mut *mut GError,
    param: &mut ExtendedLteBandPreference,
) -> gboolean {
    qmi_message_nas_get_system_selection_preference_output_get_extended_lte_band_preference(
        output,
        &mut param.masks[0],
        &mut param.masks[1],
        &mut param.masks[2],
        &mut param.masks[3],
        error,
    )
}

unsafe fn set_extended_lte_band_preference_writer(
    input: *mut QmiMessageNasSetSystemSelectionPreferenceInput,
    error: *mut *mut GError,
    param: ExtendedLteBandPreference,
) -> gboolean {
    qmi_message_nas_set_system_selection_preference_input_set_extended_lte_band_preference(
        input,
        param.masks[0],
        param.masks[1],
        param.masks[2],
        param.masks[3],
        error,
    )
}

getter_with_custom_extractor!(
    /// TODO : Documentation
    get_extended_lte_band_preference,
    get_extended_lte_band_preference_extractor,
    ExtendedLteBandPreference,
    "get_extended_lte_band_preference"
);

setter_with_writer!(
    /// TODO : Documentation
    set_extended_lte_band_preference,
    set_extended_lte_band_preference_writer,
    ExtendedLteBandPreference,
    "set_extended_lte_band_preference"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ManualNetworkSelection {
    pub mcc: u16,
    pub mnc: u16,
    pub includes_pcs_digit: bool,
}

unsafe fn get_manual_network_selection_extractor(
    output: *mut QmiMessageNasGetSystemSelectionPreferenceOutput,
    error: *mut *mut GError,
    param: &mut ManualNetworkSelection,
) -> gboolean {
    let mut my_bool = 0;
    let ret = qmi_message_nas_get_system_selection_preference_output_get_manual_network_selection(
        output,
        &mut param.mcc,
        &mut param.mnc,
        &mut my_bool,
        error,
    );
    param.includes_pcs_digit = my_bool != GFALSE;
    ret
}

getter_with_custom_extractor!(
    /// TODO : Documentation
    get_manual_network_selection,
    get_manual_network_selection_extractor,
    ManualNetworkSelection,
    "get_manual_network_selection"
);

unsafe fn get_acquisition_order_preference_extractor(
    output: *mut QmiMessageNasGetSystemSelectionPreferenceOutput,
    error: *mut *mut GError,
    param: &mut Vec<NasRadioInterface>,
) -> gboolean {
    let mut garray: *mut GArray = null_mut();
    let ret =
        qmi_message_nas_get_system_selection_preference_output_get_acquisition_order_preference(
            output,
            &mut garray,
            error,
        );
    if ret == GTRUE && !garray.is_null() {
        let values = std::slice::from_raw_parts(
            (*garray).data.cast::<QmiNasRadioInterface>(),
            (*garray).len as usize,
        );

        *param = values
            .iter()
            .copied()
            .map(|value| from_glib(value))
            .collect();
    }
    ret
}

unsafe fn set_acquisition_order_preference_writer(
    input: *mut QmiMessageNasSetSystemSelectionPreferenceInput,
    error: *mut *mut GError,
    param: &[NasRadioInterface],
) -> gboolean {
    let garray: *mut GArray = libqmi_glib_types_rs::slice_to_garray(param);
    let ret =
        qmi_message_nas_set_system_selection_preference_input_set_acquisition_order_preference(
            input, garray, error,
        );
    g_array_unref(garray);
    ret
}

getter_with_custom_extractor!(
    /// TODO : Documentation
    get_acquisition_order_preference,
    get_acquisition_order_preference_extractor,
    Vec<NasRadioInterface>,
    "get_acquisition_order_preference"
);

setter_with_writer!(
    /// TODO : Documentation
    set_acquisition_order_preference,
    set_acquisition_order_preference_writer,
    &[NasRadioInterface],
    "set_acquisition_order_preference"
);
