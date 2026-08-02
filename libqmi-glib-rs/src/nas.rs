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
#[cfg(feature = "v1_22")]
use glib_sys::{GArray, g_array_unref};
use glib_sys::{GError, GFALSE, GTRUE, gboolean};
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

macro_rules! nas_getter {
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

                rx.await.map_err(|_| QmiError::cb_droped($context))?
            }
        }
    };
}

macro_rules! nas_setter {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $qmi_function:path,
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
                let ret = unsafe { $qmi_function(input, parameter.into_glib(), &mut error) };
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
                rx.await.map_err(|_| QmiError::cb_droped($context))?
            }
        }
    }
}

macro_rules! nas_getter_with_extractor {
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

                rx.await.map_err(|_| QmiError::cb_droped($context))?
            }
        }
    };
}

macro_rules! nas_setter_with_writer {
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
                rx.await.map_err(|_| QmiError::cb_droped($context))?
            }
        }
    }
}

nas_getter!(
    /// Retrieves the preferred radio access technologies configured on the device.
    ///
    /// The returned value is a bitmask describing which radio access technologies
    /// are currently allowed by the system selection mode preference.
    get_preferred_radio_technologies,
    qmi_message_nas_get_system_selection_preference_output_get_mode_preference,
    NasRatModePreference,
    "get_preferred_radio_technologies"
);

nas_setter!(
    /// Sets the preferred radio access technologies for system selection.
    ///
    /// The supplied value is a bitmask specifying which radio access technologies
    /// the device is allowed to use when selecting a network.
    set_preferred_radio_technologies,
    qmi_message_nas_set_system_selection_preference_input_set_mode_preference,
    NasRatModePreference,
    "set_preferred_radio_technologies"
);

nas_getter!(
    /// Retrieves the current network selection preference of the device.
    ///
    /// The returned value indicates whether network selection is configured
    /// for automatic or manual operation.
    get_network_selection_mode,
    qmi_message_nas_get_system_selection_preference_output_get_network_selection_preference,
    NasNetworkSelectionPreference,
    "get_network_selection_mode"
);

/// Describes a network selection preference together with a specific mobile network.
///
/// The MCC and MNC identify the network used when manual network selection is
/// requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NasNetworkSelectionPreferenceWithLocation {
    /// Network selection preference, such as automatic or manual selection.
    pub preference: NasNetworkSelectionPreference,
    /// Mobile Country Code (MCC) identifying the country of the selected network.
    pub mcc: u16,
    /// Mobile Network Code (MNC) identifying the operator of the selected network.
    pub mnc: u16,
}

unsafe fn set_network_selection_mode_writer(
    input: *mut QmiMessageNasSetSystemSelectionPreferenceInput,
    error: *mut *mut GError,
    param: NasNetworkSelectionPreferenceWithLocation,
) -> gboolean {
    qmi_message_nas_set_system_selection_preference_input_set_network_selection_preference(
        input,
        param.preference.into_glib(),
        param.mcc,
        param.mnc,
        error,
    )
}

nas_setter_with_writer!(
    /// Sets the network selection preference of the device.
    ///
    /// When manual network selection is requested, the MCC and MNC identify
    /// the mobile network that the device should attempt to select.
    set_network_selection_mode,
    set_network_selection_mode_writer,
    NasNetworkSelectionPreferenceWithLocation,
    "set_network_selection_mode"
);

nas_getter!(
    /// Retrieves the current roaming preference of the device.
    ///
    /// The returned value indicates the type of roaming networks the device
    /// is allowed to use during system selection.
    get_roaming_preference,
    qmi_message_nas_get_system_selection_preference_output_get_roaming_preference,
    NasRoamingPreference,
    "get_roaming_preference"
);

nas_setter!(
    /// Sets the roaming preference of the device.
    ///
    /// The supplied value determines the type of roaming networks the device
    /// is allowed to use during system selection.
    set_roaming_preference,
    qmi_message_nas_set_system_selection_preference_input_set_roaming_preference,
    NasRoamingPreference,
    "set_roaming_preference"
);

nas_getter!(
    /// Retrieves the current service domain preference of the device.
    ///
    /// The returned value indicates which service domains the device is
    /// configured to use, such as circuit-switched, packet-switched, or both.
    get_service_domain_preference,
    qmi_message_nas_get_system_selection_preference_output_get_service_domain_preference,
    NasServiceDomainPreference,
    "get_service_domain_preference"
);

nas_setter!(
    /// Sets the service domain preference of the device.
    ///
    /// The supplied value determines which service domains the device should
    /// use, such as circuit-switched, packet-switched, or both.
    set_service_domain_preference,
    qmi_message_nas_set_system_selection_preference_input_set_service_domain_preference,
    NasServiceDomainPreference,
    "set_service_domain_preference"
);

nas_getter!(
    /// Retrieves the GSM/WCDMA acquisition order preference of the device.
    ///
    /// The returned value indicates which radio access technology should be
    /// preferred when acquiring service between GSM and WCDMA.
    get_gsm_wcdma_acquisition_order_preference,
    qmi_message_nas_get_system_selection_preference_output_get_gsm_wcdma_acquisition_order_preference,
    NasGsmWcdmaAcquisitionOrderPreference,
    "get_gsm_wcdma_acquisition_order_preference"
);

nas_setter!(
    /// Sets the GSM/WCDMA acquisition order preference of the device.
    ///
    /// The supplied value determines which radio access technology should be
    /// preferred when acquiring service between GSM and WCDMA.
    set_gsm_wcdma_acquisition_order_preference,
    qmi_message_nas_set_system_selection_preference_input_set_gsm_wcdma_acquisition_order_preference,
    NasGsmWcdmaAcquisitionOrderPreference,
    "set_gsm_wcdma_acquisition_order_preference"
);

nas_getter!(
    /// Retrieves the current band preference of the device.
    ///
    /// The returned value is a bitmask indicating the radio frequency bands
    /// the device is configured to use during system selection.
    get_band_preference,
    qmi_message_nas_get_system_selection_preference_output_get_band_preference,
    NasBandPreference,
    "get_band_preference"
);

nas_setter!(
    /// Sets the band preference of the device.
    ///
    /// The supplied value is a bitmask specifying the radio frequency bands
    /// the device should consider during system selection.
    set_band_preference,
    qmi_message_nas_set_system_selection_preference_input_set_band_preference,
    NasBandPreference,
    "set_band_preference"
);

nas_getter!(
    /// Retrieves the current LTE band preference of the device.
    ///
    /// The returned value is a bitmask indicating the LTE frequency bands
    /// the device should consider during system selection.
    get_lte_band_preference,
    qmi_message_nas_get_system_selection_preference_output_get_lte_band_preference,
    NasLteBandPreference,
    "get_lte_band_preference"
);

nas_setter!(
    /// Sets the LTE band preference of the device.
    ///
    /// The supplied value is a bitmask specifying the LTE frequency bands
    /// the device should consider during system selection.
    set_lte_band_preference,
    qmi_message_nas_set_system_selection_preference_input_set_lte_band_preference,
    NasLteBandPreference,
    "set_lte_band_preference"
);

nas_getter!(
    /// Retrieves the current emergency mode preference of the device.
    ///
    /// Returns `true` if emergency mode is enabled in the system selection
    /// preferences, or `false` if it is disabled.
    get_emergency_mode,
    qmi_message_nas_get_system_selection_preference_output_get_emergency_mode,
    bool,
    "get_emergency_mode"
);

nas_setter!(
    /// Sets the emergency mode preference of the device.
    ///
    /// The supplied value enables or disables emergency mode in the system
    /// selection preferences.
    set_emergency_mode,
    qmi_message_nas_set_system_selection_preference_input_set_emergency_mode,
    bool,
    "set_emergency_mode"
);

nas_getter!(
    /// Retrieves the current usage preference of the device.
    ///
    /// The returned value indicates the preferred type of service usage,
    /// such as voice-centric or data-centric operation.
    #[cfg(feature = "v1_24")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_24")))]
    get_usage_preference,
    qmi_message_nas_get_system_selection_preference_output_get_usage_preference,
    NasUsagePreference,
    "get_usage_preference"
);

nas_setter!(
    /// Sets the usage preference of the device.
    ///
    /// The supplied value specifies the preferred type of service usage,
    /// such as voice-centric or data-centric operation.
    #[cfg(feature = "v1_24")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_24")))]
    set_usage_preference,
    qmi_message_nas_set_system_selection_preference_input_set_usage_preference,
    NasUsagePreference,
    "set_usage_preference"
);

nas_getter!(
    /// Retrieves the current voice domain preference of the device.
    ///
    /// The returned value indicates the preferred domain for voice services,
    /// such as circuit-switched (CS) or packet-switched (PS) voice.
    #[cfg(feature = "v1_24")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_24")))]
    get_voice_domain_preference,
    qmi_message_nas_get_system_selection_preference_output_get_voice_domain_preference,
    NasVoiceDomainPreference,
    "get_voice_domain_preference"
);

nas_setter!(
    /// Sets the voice domain preference of the device.
    ///
    /// The supplied value specifies the preferred domain for voice services,
    /// such as circuit-switched (CS) or packet-switched (PS) voice.
    #[cfg(feature = "v1_24")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_24")))]
    set_voice_domain_preference,
    qmi_message_nas_set_system_selection_preference_input_set_voice_domain_preference,
    NasVoiceDomainPreference,
    "set_voice_domain_preference"
);

nas_getter!(
    /// Retrieves the radio access technologies disabled on the device.
    ///
    /// The returned value is a bitmask indicating which radio access
    /// technologies are explicitly disabled for system selection.
    #[cfg(feature = "v1_24")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_24")))]
    get_disabled_radio_technologies,
    qmi_message_nas_get_system_selection_preference_output_get_disabled_modes,
    NasRatModePreference,
    "get_disabled_radio_technologies"
);

/// 5G NR Standalone (SA) band preference.
///
/// The preference is represented as an extended bitmask split across eight
/// 64-bit values, indicating the NR bands considered for standalone operation.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Nr5gSaBandPreference {
    /// The eight 64-bit masks composing the 5G NR SA band preference.
    pub masks: [u64; 8],
}

#[cfg(feature = "v1_32")]
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

#[cfg(feature = "v1_32")]
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

nas_getter_with_extractor!(
    /// Retrieves the 5G NR Standalone (SA) band preference of the device.
    ///
    /// The returned value contains the extended band preference masks used
    /// for 5G NR standalone system selection.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    get_nr5g_sa_band_preference,
    get_nr5g_sa_band_preference_extractor,
    Nr5gSaBandPreference,
    "get_nr5g_sa_band_preference"
);

nas_setter_with_writer!(
    /// Sets the 5G NR Standalone (SA) band preference of the device.
    ///
    /// The supplied value contains the extended band preference masks used
    /// for 5G NR standalone system selection.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    set_nr5g_sa_band_preference,
    set_nr5g_sa_band_preference_writer,
    Nr5gSaBandPreference,
    "set_nr5g_sa_band_preference"
);

/// 5G NR Non-Standalone (NSA) band preference.
///
/// The preference is represented as an extended bitmask split across eight
/// 64-bit values, indicating the NR bands considered for non-standalone operation.
#[cfg(feature = "v1_32")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Nr5gNsaBandPreference {
    /// The eight 64-bit masks composing the 5G NR NSA band preference.
    pub masks: [u64; 8],
}

#[cfg(feature = "v1_32")]
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

#[cfg(feature = "v1_32")]
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

nas_getter_with_extractor!(
    /// Retrieves the 5G NR Non-Standalone (NSA) band preference of the device.
    ///
    /// The returned value contains the extended band preference masks used
    /// for 5G NR non-standalone system selection.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    get_nr5g_nsa_band_preference,
    get_nr5g_nsa_band_preference_extractor,
    Nr5gNsaBandPreference,
    "get_nr5g_nsa_band_preference"
);

nas_setter_with_writer!(
    /// Sets the 5G NR Non-Standalone (NSA) band preference of the device.
    ///
    /// The supplied value contains the extended band preference masks used
    /// for 5G NR non-standalone system selection.
    #[cfg(feature = "v1_32")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_32")))]
    set_nr5g_nsa_band_preference,
    set_nr5g_nsa_band_preference_writer,
    Nr5gNsaBandPreference,
    "set_nr5g_nsa_band_preference"
);

/// Extended LTE band preference.
///
/// The preference is represented as an extended bitmask split across four
/// 64-bit values, allowing LTE bands beyond those represented by the standard
/// LTE band preference mask to be specified.
#[cfg(feature = "v1_20")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_20")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ExtendedLteBandPreference {
    /// The four 64-bit masks composing the extended LTE band preference.
    pub masks: [u64; 4],
}

#[cfg(feature = "v1_20")]
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

#[cfg(feature = "v1_20")]
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

nas_getter_with_extractor!(
    /// Retrieves the extended LTE band preference of the device.
    ///
    /// The returned value contains the extended band preference masks used
    /// for LTE system selection.
    #[cfg(feature = "v1_20")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_20")))]
    get_extended_lte_band_preference,
    get_extended_lte_band_preference_extractor,
    ExtendedLteBandPreference,
    "get_extended_lte_band_preference"
);

nas_setter_with_writer!(
    /// Sets the extended LTE band preference of the device.
    ///
    /// The supplied value contains the extended band preference masks used
    /// for LTE system selection.
    #[cfg(feature = "v1_20")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_20")))]
    set_extended_lte_band_preference,
    set_extended_lte_band_preference_writer,
    ExtendedLteBandPreference,
    "set_extended_lte_band_preference"
);

/// Identifies a mobile network for manual network selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ManualNetworkSelection {
    /// Mobile Country Code (MCC) identifying the country of the network.
    pub mcc: u16,
    /// Mobile Network Code (MNC) identifying the network operator.
    pub mnc: u16,
    /// Whether the MNC includes the third PCS digit.
    ///
    /// When `true`, the MNC is interpreted as a three-digit MNC; otherwise,
    /// it is interpreted as a two-digit MNC.
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

nas_getter_with_extractor!(
    /// Retrieves the network configured for manual network selection.
    ///
    /// The returned value contains the MCC and MNC identifying the selected
    /// network, as well as whether the MNC includes the third PCS digit.
    get_manual_network_selection,
    get_manual_network_selection_extractor,
    ManualNetworkSelection,
    "get_manual_network_selection"
);

#[cfg(feature = "v1_22")]
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
    if ret == GTRUE {
        *param = garray_glib_to_vec::<QmiNasRadioInterface, _>(garray);
    }
    ret
}

#[cfg(feature = "v1_22")]
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

nas_getter_with_extractor!(
    /// Retrieves the radio interface acquisition order preference of the device.
    ///
    /// The returned list specifies the preferred order in which radio access
    /// technologies should be considered when acquiring service.
    #[cfg(feature = "v1_22")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_22")))]
    get_acquisition_order_preference,
    get_acquisition_order_preference_extractor,
    Vec<NasRadioInterface>,
    "get_acquisition_order_preference"
);

nas_setter_with_writer!(
    /// Sets the radio interface acquisition order preference of the device.
    ///
    /// The supplied slice specifies the preferred order in which radio access
    /// technologies should be considered when acquiring service.
    #[cfg(feature = "v1_22")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1_22")))]
    set_acquisition_order_preference,
    set_acquisition_order_preference_writer,
    &[NasRadioInterface],
    "set_acquisition_order_preference"
);
