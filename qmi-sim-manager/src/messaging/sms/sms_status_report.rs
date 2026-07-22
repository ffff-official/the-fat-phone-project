//! 1      TP-MTI / TP-MMS / TP-SRQ / ...
//! 1      TP-MR      - This is the reference of the SMS concerned. cf Message ID
//! 2-12   TP-RA      - cf. tp-address
//! 7      TP-SCTS    - Date of arrival at the SMSC
//! 7      TP-DT      - Delivery date
//! 1      TP-ST      - Status 0x00 means 'Short message received by the SME'
//! 0-1    TP-PI (optional)
//! 0-1    TP-PID (optional)
//! 0-1    TP-DCS (optional)
//! 0-1    TP-UDL (optional)
//! 0-140  TP-UD (optional)

use super::*;

use std::fmt;

#[derive(Debug)]
pub(crate) struct SmsStatusReportTpdu {
    tp_mti: TpduHeaderSmsStatusReport,
    tp_mr: u8,
    tp_ra: TpAddress,
    tp_scts: TpScts,
    tp_dt: TpScts,
    tp_st: TpStatus,
    tp_pi: Option<u8>,
}

/// A status report for a previously sent SMS.
///
/// Commonly referred to as a *delivery receipt*, this report indicates the
/// delivery status of a previously sent SMS message.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SmsStatusReport {
    /// The recipient's phone number.
    pub remote_address: String,
    /// The identifier of the original SMS message.
    pub message_id: u64,
    /// The time at which the SMS was delivered, expressed as a Unix timestamp in seconds.
    pub delivery_time_ts: u64,
    /// The time at which the SMS was received by the SMSC, expressed as a Unix timestamp in seconds.
    pub smsc_time_ts: u64,
    /// The delivery status of the SMS.
    pub status: TpStatus,
}
impl From<SmsStatusReportTpdu> for SmsStatusReport {
    fn from(s: SmsStatusReportTpdu) -> Self {
        Self {
            remote_address: s.tp_ra.address,
            message_id: s.tp_mr as _,
            delivery_time_ts: s.tp_dt.0,
            smsc_time_ts: s.tp_scts.0,
            status: s.tp_st,
        }
    }
}

pub(crate) fn parse_sms_status_report_tpdu(tpdu: &[u8]) -> SmResult<SmsStatusReportTpdu> {
    let mut idx = 0;
    let header = TpduHeaderSmsStatusReport::from_le_bits(*tpdu.get_result(idx)?);
    tracing::debug!("{:?}", header);
    idx += 1;

    let tp_mr = *tpdu.get_result(idx)?;
    tracing::debug!("tp_mr = {:?}", tp_mr);
    idx += 1;

    let (tp_ra, tp_ra_len) = TpAddress::from_bytes(tpdu.get_result(idx..)?)?;
    idx += tp_ra_len;
    tracing::debug!("tp_ra = {:?}", tp_ra_len);

    let tp_scts = TpScts::try_from(tpdu.get_result(idx..idx + TIMESTAMP_LEN)?)?;
    tracing::debug!("tp_scts = {:?}", tp_scts);
    idx += TIMESTAMP_LEN;

    let tp_dt = TpScts::try_from(tpdu.get_result(idx..idx + TIMESTAMP_LEN)?)?;
    tracing::debug!("tp_dt = {:?}", tp_dt);
    idx += TIMESTAMP_LEN;

    let tp_st = TpStatus::from(*tpdu.get_result(idx)?);
    tracing::debug!("tp_st = {:?}", tp_st);
    idx += 1;

    let tp_pi = tpdu.get_result(idx).ok().cloned();
    tracing::debug!("tp_pi = {:?}", tp_pi);

    Ok(SmsStatusReportTpdu {
        tp_mti: header,
        tp_mr,
        tp_ra,
        tp_scts,
        tp_dt,
        tp_st,
        tp_pi,
    })
}

#[rustfmt::skip]
impl fmt::Display for SmsStatusReportTpdu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\nTPDU")?;
        writeln!(f, "├─ TP-MTI                      : {:?}", self.tp_mti)?;
        writeln!(f, "├─ TP-MR                       : {}", self.tp_mr)?;
        writeln!(f, "├─ TP-RA                       : {}", self.tp_ra.address)?;
        writeln!(f, "├─ TP-SCTS                     : {}", self.tp_scts)?;
        writeln!(f, "├─ TP-DT                       : {}", self.tp_dt)?;
        writeln!(f, "├─ TP-ST                       : {:?}", self.tp_st)?;
        writeln!(f, "└─ TP-PI                       : 0x{:02X?}", self.tp_pi.unwrap_or_default())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TemporaryCause {
    Congestion,
    SmeBusy,
    NoResponseFromSme,
    ServiceRejected,
    QosNotAvailable,
    ErrorInSme,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PermanentCause {
    RemoteProcedureError,
    IncompatibleDestination,
    ConnectionRejectedBySme,
    NotObtainable,
    QosNotAvailable,
    NoInterworkingAvailable,
    ValidityPeriodExpired,
    DeletedByOriginatingSme,
    DeletedByServiceCenter,
    MessageDoesNotExist,
    Reserved(u8),
}

/// The current status indicated by the delivery receipt.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TpStatus {
    /// The message was successfully delivered to the recipient.
    Delivered,
    /// The message was forwarded, but delivery to the recipient could not be confirmed.
    ForwardedButUnconfirmed,
    /// The original message was replaced by the SMS service.
    Replaced,
    /// Success status reserved by 3GPP.
    SuccessReserved(u8),
    /// Success status defined by the SMSC.
    SuccessServiceCenterSpecific(u8),
    /// Temporary error. The SMSC is still trying to deliver the message.
    TemporaryFailure(TemporaryCause),
    /// Temporary failure defined by the SMSC.
    TemporaryFailureServiceCenterSpecific(u8),
    /// Permanent failure.
    PermanentFailure(PermanentCause),
    /// Permanent failure defined by the SMSC.
    PermanentFailureServiceCenterSpecific(u8),
    /// Temporary failure. The SMSC has stopped retrying.
    FinalTemporaryFailure(TemporaryCause),
    /// Final temporary failure defined by the SMSC.
    FinalTemporaryFailureServiceCenterSpecific(u8),
    /// Other values
    NotExpected,
}

impl From<u8> for TpStatus {
    fn from(value: u8) -> Self {
        use PermanentCause::*;
        use TemporaryCause::*;
        use TpStatus::*;

        match value {
            0x00 => Delivered,
            0x01 => ForwardedButUnconfirmed,
            0x02 => Replaced,

            0x03..=0x0F => SuccessReserved(value),
            0x10..=0x1F => SuccessServiceCenterSpecific(value),

            0x20 => TemporaryFailure(Congestion),
            0x21 => TemporaryFailure(SmeBusy),
            0x22 => TemporaryFailure(NoResponseFromSme),
            0x23 => TemporaryFailure(ServiceRejected),
            0x24 => TemporaryFailure(TemporaryCause::QosNotAvailable),
            0x25 => TemporaryFailure(ErrorInSme),
            0x26..=0x2F => TemporaryFailure(TemporaryCause::Reserved(value)),
            0x30..=0x3F => TemporaryFailureServiceCenterSpecific(value),

            0x40 => PermanentFailure(RemoteProcedureError),
            0x41 => PermanentFailure(IncompatibleDestination),
            0x42 => PermanentFailure(ConnectionRejectedBySme),
            0x43 => PermanentFailure(NotObtainable),
            0x44 => PermanentFailure(PermanentCause::QosNotAvailable),
            0x45 => PermanentFailure(NoInterworkingAvailable),
            0x46 => PermanentFailure(ValidityPeriodExpired),
            0x47 => PermanentFailure(DeletedByOriginatingSme),
            0x48 => PermanentFailure(DeletedByServiceCenter),
            0x49 => PermanentFailure(MessageDoesNotExist),
            0x4A..=0x4F => PermanentFailure(PermanentCause::Reserved(value)),
            0x50..=0x5F => PermanentFailureServiceCenterSpecific(value),

            0x60 => FinalTemporaryFailure(Congestion),
            0x61 => FinalTemporaryFailure(SmeBusy),
            0x62 => FinalTemporaryFailure(NoResponseFromSme),
            0x63 => FinalTemporaryFailure(ServiceRejected),
            0x64 => FinalTemporaryFailure(TemporaryCause::QosNotAvailable),
            0x65 => FinalTemporaryFailure(ErrorInSme),
            0x66..=0x6F => FinalTemporaryFailure(TemporaryCause::Reserved(value)),
            0x70..=0x7F => FinalTemporaryFailureServiceCenterSpecific(value),

            _ => NotExpected,
        }
    }
}
