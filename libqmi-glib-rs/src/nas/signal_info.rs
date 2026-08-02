use super::*;

use gobject_sys::GClosure;

/// Signal quality information for a specific radio access technology.
///
/// Each variant contains the signal measurements reported by the modem for a
/// single radio technology. Depending on the current network configuration,
/// the modem may report information for multiple radio technologies
/// simultaneously, for example LTE and NR5G during 5G NSA operation.
#[derive(Debug, Clone)]
pub enum SignalInfo {
    /// GSM signal information.
    Gsm {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,
    },
    /// CDMA2000 1x signal information.
    Cdma {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,
        /// Ec/Io signal quality measurement.
        ///
        /// The raw value is reported in units of -0.5 dB.
        ecio: i16,
    },
    /// HDR (EV-DO) signal information.
    Hdr {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,
        /// Ec/Io signal quality measurement.
        ///
        /// The raw value is reported in units of -0.5 dB.
        ecio: i16,
        /// Signal-to-interference-plus-noise ratio level.
        sinr: NasEvdoSinrLevel,
        /// Total received power (Io), in dBm.
        io: i32,
    },
    /// UMTS (WCDMA) signal information.
    Umts {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,

        /// Ec/Io signal quality measurement.
        ///
        /// The raw value is reported in units of -0.5 dB.
        ecio: i16,

        /// Received Signal Code Power (RSCP), in dBm.
        ///
        /// This value is not reported by all modems.
        #[cfg(feature = "v1_34")]
        #[cfg_attr(docsrs, doc(cfg(feature = "v1_34")))]
        rscp: Option<i16>,
    },
    /// TD-SCDMA signal information.
    TdScdma {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,
    },
    /// LTE signal information.
    Lte {
        /// Received Signal Strength Indicator (RSSI), in dBm.
        rssi: i8,
        /// Reference Signal Received Quality (RSRQ), in dB.
        rsrq: i8,
        /// Reference Signal Received Power (RSRP), in dBm.
        rsrp: i16,
        /// Signal-to-noise ratio (SNR).
        ///
        /// The raw value is reported in tenths of a dB.
        snr: i16,
    },

    /// 5G NR signal information.
    Nr5g {
        /// Reference Signal Received Power (RSRP), in dBm.
        rsrp: i16,
        /// Signal-to-noise ratio (SNR).
        ///
        /// The raw value is reported in tenths of a dB.
        snr: i16,
        /// Reference Signal Received Quality (RSRQ), in dB, when available.
        rsrq: Option<i16>,
    },
}

// TODO : Not found for the moment.
// #[derive(Debug, Clone, Copy)]
// pub struct TdScdmaSignalInfoExtended {
//     pub rssi: i32,
//     pub rscp: i32,
//     pub ecio: i32,
//     pub sinr: i32,
// }

#[rustfmt::skip]
const SETTERS: &[RadioType] = &[
    RadioType {
        capability: DmsRadioInterface::Gsm,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_gsm_rssi_delta]
    },
    RadioType {
        capability: DmsRadioInterface::Cdma20001x,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_cdma_ecio_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_cdma_rssi_delta]
    },
    RadioType {
        capability: DmsRadioInterface::Evdo,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_hdr_rssi_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_hdr_ecio_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_hdr_sinr_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_hdr_io_delta]
    },
    RadioType {
        capability: DmsRadioInterface::Umts,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_wcdma_rssi_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_wcdma_ecio_delta,
                     #[cfg(feature = "v1_34")] qmi_message_nas_config_signal_info_v2_input_set_wcdma_rscp_delta]
    },
    RadioType {
        capability: DmsRadioInterface::Lte,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_lte_rsrp_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_lte_rsrq_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_lte_rssi_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_lte_snr_delta]
    },
    RadioType {
        capability: DmsRadioInterface::Tds,
        functions: &[],
    },
    RadioType {
        capability: DmsRadioInterface::_5gnr,
        functions: &[qmi_message_nas_config_signal_info_v2_input_set_nr5g_rsrp_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_nr5g_snr_delta,
                     qmi_message_nas_config_signal_info_v2_input_set_nr5g_rsrq_delta]
    },
];

type SignalInfoCallback = Box<dyn FnMut(QmiResult<SignalInfo>)>;

#[derive(Debug)]
#[rustfmt::skip]
struct RadioType {
    capability: DmsRadioInterface,
    functions: &'static [unsafe extern "C" fn(
                *mut QmiMessageNasConfigSignalInfoV2Input, u16, *mut *mut GError) -> gboolean]
}

impl NasClient {
    /// Configures and enables NAS signal information indications.
    ///
    /// The callback is invoked whenever the modem reports updated signal
    /// information for one or more radio access technologies.
    ///
    /// The [`DmsClient`] must be used to determine the radio interfaces supported by the
    /// device so that signal reporting can be configured accordingly. [`DmsClient::get_capabilities`]
    ///
    /// # Errors
    ///
    /// Returns an error if the signal information reporting configuration cannot
    /// be applied to the modem.
    pub async fn signal_signal_info(
        &mut self,
        cb: SignalInfoCallback,
        dms_radio_interfaces: &[DmsRadioInterface],
    ) -> QmiResult<()> {
        unsafe {
            let cb_ptr = Box::into_raw(Box::new(cb)).cast();
            let handler_id = gobject_sys::g_signal_connect_data(
                self.client.as_ptr().cast(),
                c"signal-info".as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *mut gobject_sys::GObject,
                        *mut libqmi_glib_sys::QmiIndicationNasSignalInfoOutput,
                        *mut std::ffi::c_void,
                    ),
                    unsafe extern "C" fn(),
                >(signal_info_cb)),
                cb_ptr,
                Some(destroy_signal_info_data),
                0,
            );
            if handler_id == 0 {
                drop(Box::from_raw(cb_ptr.cast::<SignalInfoCallback>()));
                Err(QmiError::new("handler id = 0", "signal_signal_info"))?;
            } else {
                tracing::debug!("Receiving signal info handler id = {}", handler_id);
            }

            let input = qmi_message_nas_register_indications_input_new();

            let mut error: *mut GError = null_mut();

            if qmi_message_nas_register_indications_input_set_signal_info(input, GTRUE, &mut error)
                == GFALSE
            {
                qmi_message_nas_register_indications_input_unref(input);
                Err((error, "Cannot register indication input set signal info"))?;
            }

            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            qmi_client_nas_register_indications(
                self.client.as_ptr(),
                input,
                OPERATION_TIMEOUT,
                null_mut(),
                Some(register_indications_cb),
                Box::into_raw(Box::new(tx)).cast(),
            );

            qmi_message_nas_register_indications_input_unref(input);

            rx.await
                .map_err(|_| QmiError::cb_droped("signal_signal_info"))??;

            let radio_interfaces = dms_radio_interfaces;
            let mut accepted = Vec::new();

            for radio in SETTERS {
                if !radio_interfaces.contains(&radio.capability) {
                    continue;
                }

                for setter in radio.functions {
                    let input = qmi_message_nas_config_signal_info_v2_input_new();

                    let mut error: *mut GError = null_mut();
                    let ret = setter(input, 1, &mut error);
                    if ret == GTRUE {
                        let (tx, rx) = oneshot::channel::<QmiResult<()>>();

                        self.submit(input, tx);
                        qmi_message_nas_config_signal_info_v2_input_unref(input);

                        let ret = rx
                            .await
                            .map_err(|_| QmiError::cb_droped("signal_signal_info"))?
                            .is_ok();
                        if ret {
                            tracing::debug!(
                                "Submitting success for {:?} on {:?}",
                                setter,
                                radio.capability
                            );
                            accepted.push(setter);
                        } else {
                            tracing::debug!(
                                "Submitting error for {:?} on {:?} : {}",
                                setter,
                                radio.capability,
                                ret
                            );
                        }
                    } else {
                        tracing::debug!(
                            "Setting error for {:?} : {}",
                            setter,
                            gerror_message(error)
                        );
                        g_clear_error(&mut error);
                    }
                }
            }

            let input = qmi_message_nas_config_signal_info_v2_input_new();

            for setter in &accepted {
                let mut error: *mut GError = null_mut();
                let ret = setter(input, 1, &mut error);
                if ret == GFALSE {
                    tracing::debug!(
                        "Cannot build combined signal configuration {:?} : {}",
                        accepted,
                        gerror_message(error)
                    );
                    qmi_message_nas_config_signal_info_v2_input_unref(input);
                    Err((error, "Cannot build combined signal configuration "))?;
                }
            }
            let (tx, rx) = oneshot::channel::<QmiResult<()>>();

            self.submit(input, tx);
            qmi_message_nas_config_signal_info_v2_input_unref(input);

            rx.await
                .map_err(|_| QmiError::cb_droped("signal_signal_info"))?
        }
    }

    unsafe fn submit(
        &mut self,
        input: *mut QmiMessageNasConfigSignalInfoV2Input,
        tx: Sender<QmiResult<()>>,
    ) {
        qmi_client_nas_config_signal_info_v2(
            self.client.as_ptr(),
            input,
            OPERATION_TIMEOUT,
            null_mut(),
            Some(set_signal_info_cb),
            Box::into_raw(Box::new(tx)).cast(),
        );
    }
}

unsafe fn gerror_message(error: *mut GError) -> String {
    if error.is_null() {
        return "Unknown GLib error".to_owned();
    }

    let message = (*error).message;

    if message.is_null() {
        "Unknown GLib error".to_owned()
    } else {
        std::ffi::CStr::from_ptr(message)
            .to_string_lossy()
            .into_owned()
    }
}

unsafe extern "C" fn register_indications_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_nas_register_indications_finish(client.cast(), res, &mut error);
    if output.is_null() {
        let _drop: Result<(), Result<(), QmiError>> = sender
            .send(Err((error, "register indications cb").into()))
            .inspect_err(cannot_send);
        return;
    }

    let ret = qmi_message_nas_register_indications_output_get_result(output, &mut error);
    qmi_message_nas_register_indications_output_unref(output);

    let output: Result<(), _> = match ret {
        glib_sys::GFALSE => Err((error, "register indications cb").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn set_signal_info_cb(
    client: *mut GObject,
    res: *mut GAsyncResult,
    user_data: *mut c_void,
) {
    let sender = Box::from_raw(user_data.cast::<oneshot::Sender<QmiResult<()>>>());
    let mut error: *mut GError = null_mut();

    let output = qmi_client_nas_config_signal_info_v2_finish(client.cast(), res, &mut error);
    if output.is_null() {
        let _drop: Result<(), Result<(), QmiError>> = sender
            .send(Err((error, "nas signal info").into()))
            .inspect_err(cannot_send);
        return;
    }

    let ret = qmi_message_nas_config_signal_info_v2_output_get_result(output, &mut error);
    qmi_message_nas_config_signal_info_v2_output_unref(output);

    let output: Result<(), _> = match ret {
        glib_sys::GFALSE => Err((error, "nas signal info").into()),
        _ => Ok(()),
    };
    drop(sender.send(output).inspect_err(cannot_send));
}

unsafe extern "C" fn signal_info_cb(
    _client: *mut GObject,
    output: *mut QmiIndicationNasSignalInfoOutput,
    user_data: *mut c_void,
) {
    tracing::debug!("NAS signal-info received");

    let cb = &mut *(user_data.cast::<SignalInfoCallback>());

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let ret = qmi_indication_nas_signal_info_output_get_gsm_signal_strength(
        output, &mut rssi, &mut error,
    );
    if ret == GTRUE {
        return cb(Ok(SignalInfo::Gsm { rssi }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let mut ecio = 0;
    let ret = qmi_indication_nas_signal_info_output_get_cdma_signal_strength(
        output, &mut rssi, &mut ecio, &mut error,
    );
    if ret == GTRUE {
        return cb(Ok(SignalInfo::Cdma { rssi, ecio }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let mut ecio = 0;
    let mut sinr = 0;
    let mut io = 0;
    let ret = qmi_indication_nas_signal_info_output_get_hdr_signal_strength(
        output, &mut rssi, &mut ecio, &mut sinr, &mut io, &mut error,
    );
    if ret == GTRUE {
        return cb(Ok(SignalInfo::Hdr {
            rssi,
            ecio,
            sinr: from_glib(sinr),
            io,
        }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let mut ecio = 0;
    let ret = qmi_indication_nas_signal_info_output_get_wcdma_signal_strength(
        output, &mut rssi, &mut ecio, &mut error,
    );
    #[cfg(feature = "v1_34")]
    if ret == GTRUE {
        let mut error: *mut GError = null_mut();
        let mut rscp = 0;
        let ret =
            qmi_indication_nas_signal_info_output_get_wcdma_rscp(output, &mut rscp, &mut error);
        let rscp = if ret == GFALSE {
            g_clear_error(&mut error);
            None
        } else {
            Some(rscp)
        };
        return cb(Ok(SignalInfo::Umts { rssi, ecio, rscp }));
    }
    #[cfg(not(feature = "v1_34"))]
    if ret == GTRUE {
        return cb(Ok(SignalInfo::Umts { rssi, ecio }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let ret = qmi_indication_nas_signal_info_output_get_tdma_signal_strength(
        output, &mut rssi, &mut error,
    );
    if ret == GTRUE {
        return cb(Ok(SignalInfo::TdScdma { rssi }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rssi = 0;
    let mut rsrq = 0;
    let mut rsrp = 0;
    let mut snr = 0;
    let ret = qmi_indication_nas_signal_info_output_get_lte_signal_strength(
        output, &mut rssi, &mut rsrq, &mut rsrp, &mut snr, &mut error,
    );
    if ret == GTRUE {
        return cb(Ok(SignalInfo::Lte {
            rssi,
            rsrq,
            rsrp,
            snr,
        }));
    }
    g_clear_error(&mut error);

    let mut error: *mut GError = null_mut();
    let mut rsrp = 0;
    let mut snr = 0;
    let ret = qmi_indication_nas_signal_info_output_get_5g_signal_strength(
        output, &mut rsrp, &mut snr, &mut error,
    );
    if ret == GTRUE {
        let mut error: *mut GError = null_mut();
        let mut rsrq = 0;
        let ret = qmi_indication_nas_signal_info_output_get_5g_signal_strength_extended(
            output, &mut rsrq, &mut error,
        );
        let rsrq = if ret == GFALSE {
            g_clear_error(&mut error);
            None
        } else {
            Some(rsrq)
        };
        return cb(Ok(SignalInfo::Nr5g { rsrp, snr, rsrq }));
    }
    g_clear_error(&mut error);

    cb(Err(QmiError::new("All errors occured", "signal_info_cb")))
}

// Lauched when no more instances of GObject references QmiClientNas exists
unsafe extern "C" fn destroy_signal_info_data(data: *mut c_void, _closure: *mut GClosure) {
    tracing::debug!("destroy_signal_info_data");
    drop(Box::from_raw(data.cast::<SignalInfoCallback>()));
}
