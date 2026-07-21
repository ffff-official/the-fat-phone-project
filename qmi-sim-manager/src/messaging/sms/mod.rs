mod sms_deliver;
mod sms_status_report;
mod sms_submit;
mod tp_address;
mod tp_dcs;
mod tp_pid;
mod tp_scts;
mod tpdu_header;
mod udh;

pub use sms_deliver::{Sms, SmsWapPush};
pub use sms_status_report::{SmsStatusReport, TpStatus};
pub use udh::ConcatenatedShortMessages;

pub(crate) use sms_deliver::{SmsDeliverTpdu, SmsType, parse_sms_deliver_tpdu};
pub(crate) use sms_status_report::{SmsStatusReportTpdu, parse_sms_status_report_tpdu};
pub(crate) use sms_submit::get_sms_deliver_tpdu;

use crate::SmResult;

use super::WapPush;
use tp_address::{NumberingPlanIdentification, TpAddress, TypeOfAddress, TypeOfNumber};
use tp_dcs::{Alphabet, DataCoding, TpDcs};
use tp_pid::TpPid;
use tp_scts::{TIMESTAMP_LEN, TpScts};
use tpdu_header::{Mti, TpduHeaderSmsDeliver, TpduHeaderSmsStatusReport, TpduHeaderSmsSubmit, Vpf};

use super::SliceResultExt;

use udh::Udh;

use std::fmt;
use std::fmt::Debug;

#[derive(Debug)]
pub(crate) enum IncomingSmsTpdu {
    SmsDeliverTpdu(SmsDeliverTpdu),
    SmsStatusReportTpdu(SmsStatusReportTpdu),
}

#[derive(Debug)]
pub(crate) struct UserData {
    udh: Option<Udh>,
    pub message: Message,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Message {
    Sms(String),
    Mms(Box<WapPush>),
}

pub(crate) fn parse_incoming_sms(tpdu: &[u8]) -> SmResult<IncomingSmsTpdu> {
    let tp_mti: Mti = TpduHeaderSmsDeliver::from_le_bits(*tpdu.get_result(0)?).tp_mti();
    tracing::debug!("{:?}", tp_mti);
    use IncomingSmsTpdu::*;
    match tp_mti {
        Mti::SMS_DELIVER => Ok(SmsDeliverTpdu(parse_sms_deliver_tpdu(tpdu)?)),
        Mti::SMS_SUBMIT => Err("Unexpected sms submit tp_mti".into()),
        Mti::SMS_STATUS_REPORT => Ok(SmsStatusReportTpdu(parse_sms_status_report_tpdu(tpdu)?)),
        Mti::RESERVED => Err("Unexpected Reserved tp_mti".into()),
    }
}

#[rustfmt::skip]
impl fmt::Display for IncomingSmsTpdu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SmsDeliverTpdu(tpdu) => write!(f, "{}", tpdu),
            Self::SmsStatusReportTpdu(tpdu) => write!(f, "{}", tpdu),
        }
    }
}

#[rustfmt::skip]
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sms(content) => write!(f, "   └─ MSG                      : \"{}\"", content),
            Self::Mms(wap_push) => write!(f, "{}", wap_push),
        }
    }
}
