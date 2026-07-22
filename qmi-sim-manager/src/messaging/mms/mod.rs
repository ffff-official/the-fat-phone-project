pub(crate) mod m_notification_ind;
pub(crate) mod m_retrieve_conf;
pub(crate) mod m_send_req;

pub(crate) use super::{MmsContent, SliceResultExt};
use crate::SmResult;
pub(crate) use m_retrieve_conf::WspPartHeaders;

use num_enum::TryFromPrimitive;

use std::ffi::CStr;
use std::fmt;

/// In the encoding of the header fields, the order of the fields is not significant, except that X-Mms-Message-Type,
/// X-Mms-Transaction-ID (when present) and X-Mms-MMS-Version MUST be at the beginning of the message headers,
/// in that order, and if the PDU contains a message body the Content Type MUST be the last header field, followed by message body
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct XMmsFields {
    pub message_type: Option<MessageType>,
    pub transaction_id: Option<String>,
    pub version: Option<(u8, u8)>,
    pub from: Option<String>,
    pub message_class: Option<MessageClass>,
    pub message_size: Option<usize>,
    pub expiry: Option<usize>,
    pub priority: Option<Priority>,
    pub content_location: Option<String>,
    pub delivery_report: Option<Report>,
    pub message_id: Option<String>,
    pub date: Option<usize>,
    pub to: Option<Vec<String>>,
    pub cc: Option<Vec<String>>,
    pub subject: Option<String>,
    pub retrieve_status: Option<RetrieveStatus>,
    pub retrieve_text: Option<String>,
    pub read_report: Option<Report>,
    pub mms_status: Option<MmsStatus>,
    pub content_type: Option<ContentType>,
    pub response_status: Option<ResponseStatus>,
}

trait Check {
    fn require(&self, msg: &str) -> Result<(), String>;
    fn unexpected(&self, msg: &str);
}

impl<T> Check for Option<T> {
    fn require(&self, field_name: &str) -> Result<(), String> {
        match self {
            Some(_) => Ok(()),
            None => Err(format!("{} required", field_name)),
        }
    }
    fn unexpected(&self, field_name: &str) {
        if self.is_some() {
            tracing::warn!("Unexpected {} field", field_name);
        }
    }
}

pub(crate) fn set_xmms_fields(source: &[u8]) -> SmResult<(XMmsFields, &[u8])> {
    let mut idx = 0;
    let mut xmms_fields = XMmsFields::default();
    while idx < source.len() && parse_xmms_field(source, &mut xmms_fields, &mut idx)? {}
    if idx > source.len() {
        tracing::warn!("IDX > source.len() {} > {}", idx, source.len());
    }
    xmms_fields.version.require("version")?;
    match xmms_fields.message_type {
        None => return Err("Cannot get Message Type")?,
        Some(MessageType::MNotificationInd) => {
            xmms_fields.transaction_id.require("transaction id")?;
            xmms_fields.message_class.require("message class")?;
            xmms_fields.message_size.require("message size")?;
            xmms_fields.expiry.require("expiry")?;
            xmms_fields.content_location.require("content location")?;

            xmms_fields.content_type.unexpected("content type");
        }
        Some(MessageType::MRetrieveConf) => {
            xmms_fields.message_id.require("message id")?;
            xmms_fields.date.require("date")?;
            xmms_fields.content_type.require("content_type")?;
            xmms_fields.transaction_id.require("transaction id")?;

            xmms_fields.content_location.unexpected("content location");
        }
        _ => {}
    }
    Ok((xmms_fields, &source[idx..]))
}

fn string_until_nul(s: &[u8], idx: &mut usize) -> SmResult<String> {
    let c_str = CStr::from_bytes_until_nul(s.get_result(*idx..)?)?;
    *idx += c_str.to_bytes_with_nul().len(); // Add 0x00 byte
    Ok(c_str.to_str()?.to_string())
}

// `TRUE` indicates that traversal should continue;
// `FALSE` is sent to signal a stop (e.g., in the case of `Content-Type`).
pub(crate) fn parse_xmms_field(
    source: &[u8],
    xmms_fields: &mut XMmsFields,
    idx: &mut usize,
) -> SmResult<bool> {
    let field_name = match XMmsFieldName::try_from(*source.get_result(*idx)? & 0x7f) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Unexpected field name: {}", e);
            return Ok(true);
        }
    };
    *idx += 1;
    tracing::debug!("knowed field name detected : {:?}", field_name);
    match field_name {
        XMmsFieldName::MessageType => {
            xmms_fields.message_type = Some(MessageType::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::TransactionId => {
            xmms_fields.transaction_id = Some(string_until_nul(source, idx)?);
        }
        XMmsFieldName::Version => {
            let version = *source.get_result(*idx)?;
            xmms_fields.version = Some(((version & 0x70) >> 4, version & 0xf));
            *idx += 1;
        }
        XMmsFieldName::From => {
            let field_length = *source.get_result(*idx)? as usize;
            *idx += 1;
            let address_present_token = *source.get_result(*idx)?;
            *idx += 1;
            match address_present_token {
                0x80 => {
                    let s =
                        String::from_utf8_lossy(source.get_result(*idx..*idx + field_length - 2)?);
                    xmms_fields.from = Some(s.to_string());
                    *idx += field_length - 1;
                }
                0x81 => xmms_fields.from = None,
                _ => return Err(format!("Invalid present token : {}", address_present_token))?,
            }
        }
        XMmsFieldName::DeliveryReport => {
            xmms_fields.delivery_report = Some(Report::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::MessageClass => {
            xmms_fields.message_class = Some(MessageClass::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::Priority => {
            xmms_fields.priority = Some(Priority::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::ResponseStatus => {
            xmms_fields.response_status =
                Some(ResponseStatus::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::MessageSize => {
            let value_length = *source.get_result(*idx)? as usize;
            *idx += 1;
            let be = source.get_result(*idx..*idx + value_length)?;
            let mut sum: usize = 0;
            for v in be {
                sum = (sum << 8) | (*v as usize);
            }
            xmms_fields.message_size = Some(sum);
            *idx += value_length;
        }
        XMmsFieldName::ContentLocation => {
            xmms_fields.content_location = Some(string_until_nul(source, idx)?);
        }
        XMmsFieldName::Expiry => {
            let expiry_field_length = *source.get_result(*idx)? as usize;
            *idx += 1;
            let expiry_token_type = ExpiryTokenType::try_from(*source.get_result(*idx)?)?;
            if expiry_token_type == ExpiryTokenType::Absolute {
                *idx += expiry_field_length;
                Err("Expiry Token must be Relative in OMA spec".to_string())?;
            }
            *idx += 1;
            let value_length = *source.get_result(*idx)? as usize;
            *idx += 1;
            let be = source.get_result(*idx..*idx + value_length)?;
            let mut sum: usize = 0;
            for v in be {
                sum = (sum << 8) | (*v as usize);
            }
            xmms_fields.expiry = Some(sum);
            *idx += value_length;
        }
        XMmsFieldName::MessageId => {
            xmms_fields.message_id = Some(string_until_nul(source, idx)?);
        }
        XMmsFieldName::Date => {
            let date_len = *source.get_result(*idx)? as usize;
            *idx += 1;
            let be = source.get_result(*idx..*idx + date_len)?;
            let mut sum: usize = 0;
            for v in be {
                sum = (sum << 8) | (*v as usize);
            }
            xmms_fields.date = Some(sum);
            *idx += date_len;
        }
        XMmsFieldName::Cc => {
            let c_str = CStr::from_bytes_until_nul(source.get_result(*idx..)?)?;
            xmms_fields
                .cc
                .get_or_insert(Vec::new())
                .push(c_str.to_str()?.to_string());
            *idx += c_str.to_bytes_with_nul().len(); // Add 0x00 byte
        }
        XMmsFieldName::To => {
            let c_str = CStr::from_bytes_until_nul(source.get_result(*idx..)?)?;
            xmms_fields
                .to
                .get_or_insert(Vec::new())
                .push(c_str.to_str()?.to_string());
            *idx += c_str.to_bytes_with_nul().len(); // Add 0x00 byte
        }
        XMmsFieldName::Subject => {
            // IMPORTANT : NOT CHECKED
            xmms_fields.subject = Some(string_until_nul(source, idx)?);
        }
        XMmsFieldName::RetrieveStatus => {
            // IMPORTANT : NOT CHECKED
            xmms_fields.retrieve_status =
                Some(RetrieveStatus::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::RetrieveText => {
            // IMPORTANT : NOT CHECKED
            xmms_fields.retrieve_text = Some(string_until_nul(source, idx)?);
        }
        XMmsFieldName::ReadReport => {
            xmms_fields.read_report = Some(Report::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::XMmsStatus => {
            xmms_fields.mms_status = Some(MmsStatus::try_from(*source.get_result(*idx)?)?);
            *idx += 1;
        }
        XMmsFieldName::ContentType => {
            let content_length = *source.get_result(*idx)? as usize;
            *idx += 1;
            let content_type = WspWellKnownContentType::try_from(*source.get_result(*idx)? & 0x7f)?;
            match content_type {
                WspWellKnownContentType::WapMultipartRelated => {
                    *idx += 1;
                    let end = *idx + content_length - 1;
                    let mut start: Option<String> = None;
                    let mut mime_type: Option<String> = None;
                    while *idx < end {
                        match source.get_result(*idx)? {
                            0x8A => {
                                *idx += 1;
                                start = Some(string_until_nul(source, idx)?);
                            }
                            0x89 => {
                                *idx += 1;
                                mime_type = Some(string_until_nul(source, idx)?);
                            }
                            _ => {
                                tracing::warn!(
                                    "WapMultipartRelated - unepected value : {:?}",
                                    *idx
                                );
                                *idx += 1;
                            }
                        }
                    }
                    if let (Some(start), Some(mime_type)) = (start, mime_type) {
                        xmms_fields.content_type = Some(ContentType::WapMultipartRelated(
                            WapMultipartRelatedHeader { start, mime_type },
                        ))
                    } else {
                        tracing::error!("Cant find start or mime_type for WapMultipartRelated");
                    }
                }
                WspWellKnownContentType::ImageJpeg => {
                    // IMPORTANT : NOT CHECKED
                    *idx += 1;
                    xmms_fields.content_type = Some(ContentType::ImageJpeg);
                }
                WspWellKnownContentType::TextPlain => {
                    // IMPORTANT : NOT CHECKED
                    *idx += 1;
                    xmms_fields.content_type = Some(ContentType::TextPlain);
                }
                _ => {
                    tracing::warn!("Unimplemented content type : {:?}", content_type);
                    *idx += content_length;
                }
            }
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum XMmsFieldName {
    Cc = 0x02,              // M-Retrieve.conf OPT
    ContentLocation = 0x03, // M-Notification.ind MANDATORY
    ContentType = 0x04,     // MANDATORY
    Date = 0x05,            // M-Retrieve.conf MANDATORY
    DeliveryReport = 0x06,  // OPT
    Expiry = 0x08,          // M-Notification.ind MANDATORY
    From = 0x09,            // OPT
    ResponseStatus = 0x12,  // M-Send.conf MANDATORY
    XMmsStatus = 0x15,      // M-Delivery-Ind
    MessageClass = 0x0A,    // M-Notification.ind MANDATORY
    MessageId = 0x0B,       // M-Retrieve.conf MANDATORY
    MessageType = 0x0C,
    Version = 0x0D,        // MANDATORY
    MessageSize = 0x0E,    // M-Notification.ind MANDATORY
    Priority = 0x0F,       // OPT
    ReadReport = 0x10,     // OPT
    Subject = 0x16,        // OPT
    To = 0x17,             // M-Retrieve.conf OPT
    TransactionId = 0x18,  // M-Notification.ind && M.send.req MANDATORY
    RetrieveStatus = 0x19, // M-Retrieve.conf OPT
    RetrieveText = 0x1A,   // M-Retrieve.conf OPT
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MessageType {
    /// Sending an MMS to the server
    MSendReq = 0x80,
    /// Shipping Confirmation
    MSendConf,
    /// The network tells you: "an MMS is available"
    MNotificationInd,
    MNotifyRespInd,
    /// The server sends you the full MMS
    MRetrieveConf,
    MAcknowledgeInd,
    /// Delivery Receipt
    MDeliveryInd,
    MReadRecInd,
    /// Acknowledgment
    MReadOrigInd,
    MForwardReq,
    MForwardConf,
    MMboxStoreReq,
    MMboxStoreConf,
    MMboxViewReq,
    MMboxViewConf,
    MMboxUploadReq,
    MMboxUploadConf,
    MMboxDeleteReq,
    MMboxDeleteConf,
    MMboxDescr,
    MDeleteReq,
    MDeleteConf,
    MCancelReq,
    MCancelConf,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum MessageClass {
    Personal = 0x80,
    Advertisement,
    Informational,
    Auto,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Priority {
    Low = 0x80,
    Normal,
    High,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Report {
    /// Reception ACK
    Yes = 0x80,
    No = 0x81,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum ExpiryTokenType {
    Absolute = 0x80,
    Relative = 0x81,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum RetrieveStatus {
    Ok = 0x80,
    ErrorTransientFailure = 0xC0,
    ErrorTransientMessageNotFound,
    ErrorTransientNetworkProblem,
    ErrorPermanentFailure = 0xE0,
    ErrorPermanentServiceDenied,
    ErrorPermanentMessageNotFound,
    ErrorPermanentContentUnsupported,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub(crate) enum WspWellKnownContentType {
    Any = 0x00,
    TextPlain = 0x03,
    MultipartMixed = 0x0C,
    MultipartAlternative = 0x0F,
    ImageGif = 0x1D,
    ImageJpeg = 0x1E,
    ImagePng = 0x20,
    ApplicationXml = 0x27,
    TextXml = 0x28,
    WapMultipartMixed = 0x23,
    WapMultipartAlternative = 0x26,
    WapMultipartRelated = 0x33,
    ApplicationXhtmlXml = 0x3B,
    WapMmsMessage = 0x3E,
    OctetStream = 0x5A,
    Json = 0x5B,
}

/// The response status returned by the MMSC after an MMS submission.
///
/// The values are defined by the MMS specification.
///
/// Status codes are grouped into:
/// - `0x80`: Success.
/// - `0x81..=0x88`: Permanent errors.
/// - `0xC0..=0xC4`: Transient errors.
/// - `0xE0..=0xEB`: Permanent failures.
#[allow(missing_docs)]
#[repr(u8)]
#[derive(Debug, TryFromPrimitive, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ResponseStatus {
    /// The MMS was accepted by the MMSC.
    Ok = 0x80,

    ErrorUnspecified = 0x81,
    ErrorServiceDenied = 0x82,
    ErrorMessageFormatCorrupt = 0x83,
    ErrorSendingAddressUnresolved = 0x84,
    ErrorMessageNotFound = 0x85,
    ErrorNetworkProblem = 0x86,
    ErrorContentNotAccepted = 0x87,
    ErrorUnsupportedMessage = 0x88,

    ErrorTransientFailure = 0xC0,
    ErrorTransientSendingAddressUnresolved = 0xC1,
    ErrorTransientMessageNotFound = 0xC2,
    ErrorTransientNetworkProblem = 0xC3,
    ErrorTransientPartialSuccess = 0xC4,

    ErrorPermanentFailure = 0xE0,
    ErrorPermanentServiceDenied = 0xE1,
    ErrorPermanentMessageFormatCorrupt = 0xE2,
    ErrorPermanentSendingAddressUnresolved = 0xE3,
    ErrorPermanentMessageNotFound = 0xE4,
    ErrorPermanentContentNotAccepted = 0xE5,
    ErrorPermanentReplyChargingLimitationsNotMet = 0xE6,
    ErrorPermanentReplyChargingRequestNotAccepted = 0xE7,
    ErrorPermanentReplyChargingForwardingDenied = 0xE8,
    ErrorPermanentReplyChargingNotSupported = 0xE9,
    ErrorPermanentAddressHidingNotSupported = 0xEA,
    ErrorPermanentLackOfPrepaid = 0xEB,

    /// Returned by the library when the received status code is unknown.
    Unknown = 0xFF,
}

#[derive(Debug, TryFromPrimitive, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MmsStatus {
    /// The MMS expired before it could be retrieved by the recipient.
    Expired = 0x80,
    /// The MMS was successfully retrieved by the recipient.
    Retrieved = 0x81,
    /// The MMS was rejected by the recipient's client.
    Rejected = 0x82,
    /// The recipient's client has deferred retrieval of the MMS.
    Deferred = 0x83,
    /// Reserved for versioning purposes.
    /// This value must not appear in an M-Delivery.ind PDU.
    Unrecognised = 0x84,
    /// The delivery status could not be determined.
    Indeterminate = 0x85,
    /// The MMS was forwarded without being retrieved.
    Forwarded = 0x86,
    /// The recipient was unreachable.
    Unreachable = 0x87,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ContentType {
    WapMultipartRelated(WapMultipartRelatedHeader),
    ImageJpeg,
    TextPlain,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct WapMultipartRelatedHeader {
    start: String,
    mime_type: String,
}

#[rustfmt::skip]
impl fmt::Display for XMmsFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let version = match self.version {
            Some(v) => (v.0, v.1),
            None => (0, 0)
        };
        writeln!(f, "         ├─ Message-Type       : {}", display_option(&self.message_type))?;
        if let Some(transaction_id) = &self.transaction_id {
            writeln!(f, "         ├─ Transaction-ID     : {}", transaction_id)?;
        }
        if let Some(from) = &self.from {
            writeln!(f, "         ├─ From               : {}", from)?;
        }
        if let Some(message_class) = &self.message_class {
            writeln!(f, "         ├─ Message-Class      : {:?}", message_class)?;
        }
        if let Some(message_size) = &self.message_size {
            writeln!(f, "         ├─ Message-Size       : {} bytes", message_size)?;
        }
        if let Some(expiry) = &self.expiry {
            writeln!(f, "         ├─ Expiry             : {}", format_duration(*expiry))?;
        }
        if let Some(priority) = &self.priority {
            writeln!(f, "         ├─ Priority           : {:?}", priority)?;
        }
        if let Some(content_location) = &self.content_location {
            writeln!(f, "         ├─ Content-Location   : {}", content_location)?;
        }
        if let Some(delivery_report) = &self.delivery_report {
            writeln!(f, "         ├─ Delivery-Report    : {:?}", delivery_report)?;
        }
        if let Some(message_id) = &self.message_id {
            writeln!(f, "         ├─ Message-ID         : {}", message_id)?;
        }
        if let Some(date) = &self.date {
            writeln!(f, "         ├─ Date               : {}", date)?;
        }
        if let Some(to) = &self.to {
            writeln!(f, "         ├─ To                 : {:?}", to)?;
        }
        if let Some(cc) = &self.cc {
            writeln!(f, "         ├─ Cc                 : {:?}", cc)?;
        }
        if let Some(subject) = &self.subject {
            writeln!(f, "         ├─ Subject            : {}", subject)?;
        }
        if let Some(retrieve_status) = &self.retrieve_status {
            writeln!(f, "         ├─ Retrieve-Status    : {:?}", retrieve_status)?;
        }
        if let Some(retrieve_text) = &self.retrieve_text {
            writeln!(f, "         ├─ Retrieve-Text      : {}", retrieve_text)?;
        }
        if let Some(read_report) = &self.read_report {
            writeln!(f, "         ├─ Read-Report        : {:?}", read_report)?;
        }
        if let Some(content_type) = &self.content_type {
            writeln!(f, "         ├─ Content-Type       : {:02X?}", content_type)?;
        }
        if let Some(response_status) = &self.response_status {
            writeln!(f, "         ├─ Respnse-Status     : {:?}", response_status)?;
        }
        if let Some(mms_status) = &self.mms_status {
            writeln!(f, "         ├─ Mms-Status         : {:?}", mms_status)?;
        }
        writeln!(f, "         └─ MMS-Version        : {}.{}", version.0, version.1)
    }
}

fn display_option<T: fmt::Debug>(value: &Option<T>) -> String {
    match value {
        Some(v) => format!("{v:?}"),
        None => "MANDATORY".into(),
    }
}

fn format_duration(seconds: usize) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m {seconds}s")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

#[rustfmt::skip]
impl fmt::Display for WspWellKnownContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WspWellKnownContentType::Any => "*/*",
            WspWellKnownContentType::TextPlain => "text/plain",
            WspWellKnownContentType::MultipartMixed => "multipart/mixed",
            WspWellKnownContentType::MultipartAlternative => "multipart/alternative",
            WspWellKnownContentType::ImageGif => "image/gif",
            WspWellKnownContentType::ImageJpeg => "image/jpeg",
            WspWellKnownContentType::ImagePng => "image/png",
            WspWellKnownContentType::ApplicationXml => "application/xml",
            WspWellKnownContentType::TextXml => "text/xml",
            WspWellKnownContentType::WapMultipartMixed => "application/vnd.wap.multipart.mixed",
            WspWellKnownContentType::WapMultipartAlternative => "application/vnd.wap.multipart.alternative",
            WspWellKnownContentType::WapMultipartRelated => "application/vnd.wap.multipart.related",
            WspWellKnownContentType::ApplicationXhtmlXml => "application/xhtml+xml",
            WspWellKnownContentType::WapMmsMessage => "application/vnd.wap.mms-message",
            WspWellKnownContentType::OctetStream => "application/octet-stream",
            WspWellKnownContentType::Json => "application/json",
        };
        write!(f, "{s}")
    }
}
