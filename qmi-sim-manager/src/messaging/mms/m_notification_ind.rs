//! ```text
//! User Data (UD)
//!  ├─── UDH
//!  │     └─── Port Addressing (2948/9200)
//!  └─── WAP Push
//!        ├─── WSP headers
//!        │     ├─ Transaction-ID
//!        │     ├─ PDU Type
//!        │     ├─ Content-Type
//!        │     └─ X-Wap-Application-Id
//!        └─── MMS notification (M-Notification.ind)
//!              ├─ X-Mms-Message-Type
//!              ├─ X-Mms-Transaction-ID
//!              ├─ X-Mms-MMS-Version
//!              ├─ From
//!              ├─ X-Mms-Message-Class
//!              ├─ X-Mms-Expiry
//!              └─ X-Mms-Content-Location  (other fields are possible...)
//!
//! EXAMPLE OF COMPLETE TPDU UN CASE OF WAP PUSH NOTIFICATION :
//! ----------------------------------------------------------
//!
//! 44 0B 91 33 46 21 68 06 F3 00 04 62 70 30 91 41 31 80 8A 06 05 04 0B 84 23 F0 00 06 04 01 BE AF 84 8C 82 98 6E 56
//! 6E 47 53 58 63 43 45 66 47 65 34 67 41 6D 75 66 71 36 39 41 00 8D 92 89 18 80 2B 33 33 36 34 31 32 38 36 36 30 33
//! 2F 54 59 50 45 3D 50 4C 4D 4E 00 86 81 8A 80 8F 81 8E 03 02 CA 10 83 68 74 74 70 3A 2F 2F 32 31 33 2E 32 32 38 2E
//! 33 2E 34 35 2F 6D 6D 73 2E 70 68 70 3F 6E 56 6E 47 53 58 63 43 45 66 47 65 34 67 41 6D 75 66 71 36 39 41 00 88 05
//! 81 03 09 3A 80
//!
//! -------- 44 0B 91 33 46 21 68 06 F3 00 04 62 70 30 91 41 31 80 8A => TPDU Header where `tp-udhi = 1`
//!
//! ### BEGIN OF USER DATA ###
//!
//! -------- 06 05 04 0B 84 23 F0 => UDH Port Addressing (2948/9200)
//!
//! WSP
//! ------- 00 06 04 01 BE AF 84
//! 00                WSP Transaction-ID
//! 06                WSP PDU Type = Push
//! 04                WSP Headers Length = 4
//! 01 BE             Content-Type = application/vnd.wap.mms-message
//! AF 84             X-Wap-Application-Id = MMS UA
//!
//! MMS
//! ------- 8C 82 98
//! 8C 82             X-Mms-Message-Type = m-notification-ind
//! 98 <string> 00    X-Mms-Transaction-ID = nVnGSXcCEfGe4gAmufq69B
//! ------- 6E 56 6E 47 53 58 63 43 45 66 47 65 34 67 41 6D 75 66 71 36 39 42 00
//! -> nVnGSXcCEfGe4gAmufq69A
//! ------- 8D 92 89 18 80
//! 8D 92            X-Mms-MMS-Version = 1.2
//! 89 18 80 <addr>00 From
//! 18               Length of From field (24 ?)
//! 80               address-present-token                  : 01 81 instead = Insert-address-token
//! ------- 2B 33 33 36 34 31 32 38 36 36 30 33 2F 54 59 50 45 3D 50 4C 4D 4E 00
//! -> +33641286603/TYPE=PLMN
//! ------- 86 81 8A 80 8F 81 8E 03 02 CA 10 83
//! 86               X-Mms-Delivery-Report
//! 81               No
//! 8A               Message Class
//! 80               Personal
//! 8F               X-Mms-Priority
//! 81               Normal
//! 8E               X-Mms-Message-size
//! 03               Value-length = 3
//! 02 CA 10         octets in BE
//! 83               X-Mms-Content-Location
//! ------- 68 74 74 70 3A 2F 2F 32 31 33 2E 32 32 38 2E 33 2E 34 35 2F 6D 6D 73 2E 70 68 70 3F 6E 56 6E 47 53 58 63 43 45 66 47 65 34 67 41 6D 75 66 71 36 39 42 00
//! -> http://213.228.3.45/mms.php?nVnGSXcCEfGe4gAmufq69B
//! ------- 88 05 81 03 09 3A 80
//! 88               X-Mms-Expiry
//! 05               Value-length = 5
//! 81               Relative-token
//! 03               Long-integer length = 3
//! 09 3A 80         Delta-seconds = 0x093A80 (7 days)
//!
//! Transaction-ID: nVnGSXcCEfGe4gAmufq69B
//! MMS-Version: 1.2
//! From: +33641286603/TYPE=PLMN
//! Message-Class: Personal
//! Priority: Normal
//! Expiry: 7 days
//! Content-Location: http://213.228.3.45/mms.php?nVnGSXcCEfGe4gAmufq69B
//! ```
use crate::Error;

use super::{SliceResultExt, XMmsFields, set_xmms_fields};

use num_enum::TryFromPrimitive;

use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct WapPush {
    content_type: ContentType,
    wsp_header: WspHeader,
    x_wap_application_id: XWapApplicationId,
    pub(crate) xmms_fields: XMmsFields,
}

impl TryFrom<&[u8]> for WapPush {
    type Error = Error;

    fn try_from(source: &[u8]) -> Result<Self, Self::Error> {
        let transaction_id = *source.get_result(0)?;
        tracing::debug!("transaction_id = {}", transaction_id);
        let pdu_type = PduType::try_from(*source.get_result(1)? & 0x7f)?;
        if pdu_type != PduType::Push {
            Err(format!("Not a WAP PUSH : Got {:?}", pdu_type))?;
        }
        let header_length = *source.get_result(2)?;
        if header_length != 4 {
            Err("Header length must have len of 4")?;
        }
        let mut idx = 3;
        let content_type_length = *source.get_result(idx)?;
        if content_type_length != 1 {
            Err(format!(
                "Content type is not a Well Know Value : Got len {:?}",
                content_type_length
            ))?;
        }
        idx += 1;
        let content_type = ContentType::try_from(*source.get_result(idx)? & 0x7f)?;
        tracing::debug!("content-type : {:?}", content_type);
        if content_type != ContentType::ApplicationVndWapMmsMessage {
            Err(format!("Not a WAP MMS MESSAGE : Got {:?}", content_type))?;
        }
        idx += 1;
        let wsp_header = WspHeader::try_from(*source.get_result(idx)? & 0x7f)?;
        if wsp_header != WspHeader::XWapApplicationId {
            Err(format!("Unexpected WSP header : Got {:?}", wsp_header))?;
        }
        idx += 1;
        let x_wap_application_id = XWapApplicationId::try_from(*source.get_result(idx)? & 0x7f)?;
        if x_wap_application_id != XWapApplicationId::XWapApplicationMmsUa {
            Err(format!(
                "Unexpected application id : Got {:?}",
                x_wap_application_id
            ))?;
        }
        idx += 1;
        Ok(Self {
            content_type,
            wsp_header,
            x_wap_application_id,
            xmms_fields: set_xmms_fields(&source[idx..])?.0,
        })
    }
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum PduType {
    Connect = 0x01,
    ConnectReply = 0x02,
    Redirect = 0x03,
    Reply = 0x04,
    Disconnect = 0x05,
    Push = 0x06, // WAP PUSH
    ConfirmedPush = 0x07,
    Suspend = 0x08,
    Resume = 0x09,
    Get = 0x40,
    Options = 0x41,
    Head = 0x42,
    Delete = 0x43,
    Trace = 0x44,
    Post = 0x60,
    Put = 0x61,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum WspHeader {
    XWapApplicationId = 0x2F,
    XWapContentUri = 0x30,
    XWapInitiatorUri = 0x31,
    AcceptApplication = 0x32,
    PushFlag = 0x34,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum ContentType {
    ApplicationVndWapMmsMessage = 0x3E,
    TextVndWapSi = 0x2E,
    TextVndWapSl = 0x30,
}

#[derive(Debug, TryFromPrimitive, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum XWapApplicationId {
    XWapApplicationMmsUa = 0x04,
    XWapApplicationWmlUa = 0x05,
    XWapApplicationWtaUa = 0x06,
    XWapApplicationMtaUa = 0x07,
}

#[rustfmt::skip]
impl fmt::Display for WapPush {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "   └─ WAP Push")?;
        writeln!(f, "      ├─ Content-Type          : {:?}", self.content_type)?;
        writeln!(f, "      ├─ WSP Header            : {:?}", self.wsp_header)?;
        writeln!(f, "      ├─ X-Wap-Application-Id  : {:?}", self.x_wap_application_id)?;
        writeln!(f, "      └─ MMS Fields")?;
        write!(f, "{}", self.xmms_fields)
    }
}
