//! See <http://rfc.nop.hu/sms/default.htm>
//! ```text
//! TPDU SMS DELIVER
//! ├─ TP-MTI / first byte (TPDU Header)
//! ├─ TP-OA   (Originating Address) cf. length / type and phone number
//! ├─ TP-PID  (Protocol Identifier)
//! ├─ TP-DCS  (Data Coding Scheme)
//! ├─ TP-SCTS (Timestamp)
//! ├─ TP-UDL  (User Data Length)
//! └─ TP-UD   (User Data)
//!     ├─ UDH (optional, if TP-UDHI=1)
//!     └─ User payload
//! ```
use crate::Error;

use super::*;
use udh::ConcatenatedShortMessages;

use gsm7_rs::Gsm7Reader;

use std::{fmt, io};

#[derive(Debug)]
pub(crate) struct SmsDeliverTpdu {
    tp_mti: TpduHeaderSmsDeliver,
    tp_oa: TpAddress,
    tp_pid: TpPid,
    tp_dcs: TpDcs,
    tp_scts: TpScts,
    tp_udl: usize,
    tp_ud: UserData,
}

/// Represents a received SMS message.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Sms {
    /// The sender's phone number.
    pub remote_address: String,
    /// The SMSC timestamp, expressed as a Unix timestamp in seconds.
    pub smsc_time_ts: u64,
    /// Concatenation information for multipart SMS messages.
    ///
    /// This is `None` for single-part messages.
    pub multipart: Option<ConcatenatedShortMessages>,
    /// The decoded SMS message content.
    pub content: String,
}

/// Indicates that an MMS is available for download.
///
/// The MMS can be retrieved from the URL stored in [`SmsWapPush::content_location`].
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SmsWapPush {
    /// The sender's phone number.
    pub remote_address: String,
    /// The SMSC timestamp, expressed as a Unix timestamp in seconds.
    pub smsc_time_ts: u64,
    /// The source UDP port of the WAP Push message.
    pub source_port: u16,
    /// The destination UDP port of the WAP Push message.
    pub destination_port: u16,
    /// The URL from which the MMS can be downloaded.
    pub content_location: String,
}

pub(crate) enum SmsType {
    Sms(Sms),
    SmsWapPush(SmsWapPush),
}

impl TryFrom<SmsDeliverTpdu> for SmsType {
    type Error = &'static str;

    fn try_from(s: SmsDeliverTpdu) -> Result<Self, Self::Error> {
        match s.tp_ud.message {
            Message::Sms(content) => {
                let multipart = s
                    .tp_ud
                    .udh
                    .map(|udh| {
                        if let udh::UdhContent::ConcatenatedShortMessages(m) = udh.content {
                            Ok(m)
                        } else {
                            Err("Missing ConcatenatedShortMessages type in multipart SMS")
                        }
                    })
                    .transpose()?;
                Ok(Self::Sms(Sms {
                    remote_address: s.tp_oa.address,
                    smsc_time_ts: s.tp_scts.0,
                    multipart,
                    content,
                }))
            }
            Message::Mms(mms) => {
                let ports = s
                    .tp_ud
                    .udh
                    .map(|udh| {
                        if let udh::UdhContent::ApplicationPortAddressing16(m) = udh.content {
                            Ok(m)
                        } else {
                            Err("Missing ApplicationPortAddressing16 in MMS")
                        }
                    })
                    .ok_or("Missing udh in Wap Push")??;

                Ok(Self::SmsWapPush(SmsWapPush {
                    remote_address: s.tp_oa.address,
                    smsc_time_ts: s.tp_scts.0,
                    source_port: ports.source_port,
                    destination_port: ports.destination_port,
                    content_location: mms.xmms_fields.content_location.ok_or("Missing location")?,
                }))
            }
        }
    }
}

pub(crate) fn parse_sms_deliver_tpdu(tpdu: &[u8]) -> SmResult<SmsDeliverTpdu> {
    let mut idx = 0;
    let header = TpduHeaderSmsDeliver::from_le_bits(*tpdu.get_result(idx)?);
    tracing::debug!("{:?}", header);
    idx += 1;

    let (tp_oa, tp_oa_len) = TpAddress::from_bytes(tpdu.get_result(idx..)?)?;
    idx += tp_oa_len;
    tracing::debug!("tp_oa = {:?}", tp_oa);

    let tp_pid = TpPid::from(*tpdu.get_result(idx)?);
    tracing::debug!("tp_pid = {:?}", tp_pid);

    idx += 1;
    let tp_dcs = TpDcs::try_from(*tpdu.get_result(idx)?)?;
    tracing::debug!("tp_dsc = {:?}", tp_dcs);

    idx += 1;
    let tp_scts = TpScts::try_from(tpdu.get_result(idx..idx + TIMESTAMP_LEN)?)?;

    tracing::debug!("tp_scts = {:?}", tp_scts);

    idx += TIMESTAMP_LEN;
    let user_data_len = *tpdu.get_result(idx)? as usize;

    tracing::debug!("User Data Len = {}", user_data_len);

    idx += 1;
    let udh = if TpduHeaderSmsDeliver::tp_udhi(&header) == 1 {
        let udh = Udh::try_from(tpdu.get_result(idx..)?)?;
        tracing::debug!("UDH from tpdu = {:?}", udh);

        idx += udh.len;
        Some(udh)
    } else {
        None
    };

    let message = match tp_dcs {
        TpDcs::DataCoding(DataCoding {
            alphabet: Alphabet::Gsm7,
            ..
        }) => {
            let (offset, text_septets) = if let Some(udh) = udh {
                let udh_bits = udh.len * 8;
                let offset_bits = (7 - (udh_bits % 7)) % 7;
                let consumed_septets = (udh_bits + offset_bits) / 7;

                let text_septets = user_data_len - consumed_septets;
                (offset_bits, text_septets)
            } else {
                (0, user_data_len)
            };

            let reader = Gsm7Reader::new_with_bit_offset(
                io::Cursor::new(tpdu.get_result(idx..)?),
                offset as _,
            )
            .map_err(Error::Gsm7Error)?;
            let message = reader
                .into_iter()
                .take(text_septets)
                .collect::<io::Result<String>>()
                .map_err(Error::Gsm7Error)?;
            Message::Sms(message)
        }
        TpDcs::DataCoding(DataCoding {
            alphabet: Alphabet::Ucs2,
            ..
        }) => {
            let u8_slice = tpdu.get_result(idx..)?;

            let u16s: Vec<u16> = u8_slice
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();

            Message::Sms(String::from_utf16(&u16s)?)
        }
        TpDcs::DataCoding(DataCoding {
            alphabet: Alphabet::Data8Bit,
            ..
        }) => Message::Mms(Box::new(WapPush::try_from(tpdu.get_result(idx..)?)?)),
        _ => unimplemented!(),
    };
    Ok(SmsDeliverTpdu {
        tp_mti: header,
        tp_oa,
        tp_pid,
        tp_dcs,
        tp_scts,
        tp_udl: user_data_len,
        tp_ud: UserData { udh, message },
    })
}

#[rustfmt::skip]
impl fmt::Display for SmsDeliverTpdu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\nTPDU")?;
        writeln!(f, "├─ TP-MTI                      : {:?}", self.tp_mti)?;
        writeln!(f, "├─ TP-OA                       : {}", self.tp_oa.address)?;
        writeln!(f, "├─ TP-PID                      : {:?}", self.tp_pid)?;
        writeln!(f, "├─ TP-DCS                      : {:?}", self.tp_dcs)?;
        writeln!(f, "├─ TP-SCTS                     : {}", self.tp_scts)?;
        writeln!(f, "├─ TP-UDL                      : {} units", self.tp_udl)?;
        writeln!(f, "└─ TP-UD")?;
        if let Some(udh) = self.tp_ud.udh {
            writeln!(f, "{}", udh)?
        }
        writeln!(f, "{}", self.tp_ud.message)
    }
}

#[cfg(test)]
#[path = "../../../tests/sms/sms_deliver.rs"]
mod tests;
