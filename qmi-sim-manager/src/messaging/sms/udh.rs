//! See <https://en.wikipedia.org/wiki/User_Data_Header>
//! ```text
//! Presence of User Data Header is indicated by the TP-UDHI (Transfer Layer Protocol User Data Header Indicator)
//! bit - 6th bit of the first octet of the GSM 03.40 or 3GPP 23.040 message.
//!
//! If UDH is present, it is at the beginning of the TP-UD (TP-User Data) field and it always starts with an UDHL (UDH Length) octet.
//! The rest of UDH is a sequence of Tag-Length-Value triplets. The Tag is named Information-Element-Identifier (IEI) and is always 1 octet long,
//! the Length field is always 1 octet long and it specifies the length of the Information Element Data.
//!
//!
//! TP-UD (User Data)
//! ┌──────┬──────┬──────┬──────────────────────┐
//! │ UDHL │ IEI  │ IEDL │ IE data              │
//! ├──────┼──────┼──────┼──────────────────────┼
//! │  06  │  05  │  04  │ 0B 84 23 F0          │
//! └──────┴──────┴──────┴──────────────────────┘
//!    │      │      │           │
//!    │      │      │           └─ data for this IE
//!    │      │      └─ length of IE data
//!    │      └─ IE type
//!    └─ total UDH length AFTER this byte
//!
//! MMS/WAP Push Example
//!
//! 06 05 04 0B 84 23 F0
//! │  │  │  └────┬─────┘
//! │  │  │       ├─ dst port 0x0B84 = 2948
//! │  │  │       └─ src port 0x23F0 = 9200
//! │  │  └─ IEDL = 4
//! │  └─ IEI = 0x05, 16-bit port addressing
//! └─ UDHL = 6
//!
//! Example: concatenated SMS, 8-bit reference
//!
//! TP-UD (User Data)
//! ┌──────┬──────┬──────┬────────────────┬─────────────────────┐
//! │ UDHL │ IEI  │ IEDL │ IE data        │ GSM7/User payload   │
//! ├──────┼──────┼──────┼────────────────┼─────────────────────┤
//! │  05  │  00  │  03  │ 7A 02 01       │ ...                 │
//! └──────┴──────┴──────┴────────────────┴─────────────────────┘
//!    │      │      │       │  │  │
//!    │      │      │       │  │  └─ current part = 1
//!    │      │      │       │  └──── total parts = 2
//!    │      │      │       └─────── reference = 0x7A
//!    │      │      └─ IEDL = 3
//!    │      └─ IEI = 0x00, concatenated SMS, 8-bit ref
//!    └─ UDHL = 5
//!
//! ***WARNING*** UDH uses space intended for the message payload. The longer the UDH, the less space remains for the message text or other contents.
//! When TP-UD contains seven-bit data, the length of TP-UD is given in septets (but UDHL is still in octets, so length in septets is (UDHL + 1) * 8 / 7)
//! and the UDH is filled to a septet boundary with 0-6 zero bits.
//! ```
use crate::SmError;

use super::SliceResultExt;

use num_enum::TryFromPrimitive;

use std::fmt;

#[derive(Debug, Copy, Clone)]
pub(crate) struct Udh {
    pub content: UdhContent,
    pub len: usize,
}

impl TryFrom<&[u8]> for Udh {
    type Error = SmError;

    fn try_from(source: &[u8]) -> Result<Self, Self::Error> {
        let content = UdhContent::try_from(source)?; // First parse and check UDH
        Ok(Self {
            content,
            len: *source.get_result(0)? as usize + 1,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum UdhContent {
    ConcatenatedShortMessages(ConcatenatedShortMessages),
    ApplicationPortAddressing16(ApplicationPortAddressing16),
}

/// The ConcatenatedShortMessages structure indicates the position of the current SMS within a concatenated SMS message.
#[derive(Debug, Copy, Clone)]
pub struct ConcatenatedShortMessages {
    /// All message parts share the same reference number.
    pub reference: u16,
    /// Message part indices start at 1.
    pub message_number: u8,
    /// The total number of message parts.
    pub total_messages: u8,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ApplicationPortAddressing16 {
    pub(crate) destination_port: u16,
    pub(crate) source_port: u16,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
enum Iei {
    /// Concatenated short messages, 8-bit reference number - SMS Control 3 no repeat
    ConcatenatedShortMessages8bitReference = 0x00,
    /// Concatenated short messages, 16-bit reference number - SMS Control 3 no repeat
    ConcatenatedShortMessages16bitReference = 0x08,
    /// Application port addressing scheme, 16 bit address - SMS Control 4 no repeat
    ApplicationPortAddressing16bitAddress = 0x05,
}

impl TryFrom<&[u8]> for UdhContent {
    type Error = SmError;

    fn try_from(source: &[u8]) -> Result<Self, Self::Error> {
        let udh_length = *source.get_result(0)?;
        let iei = Iei::try_from(*source.get_result(1)?)?;
        tracing::debug!("EIE = {:?}", iei);
        let ieidl = *source.get_result(2)?; // IEDL (Information Element Data Length) specifies the number of bytes contained in the data portion of the Information Element.
        match iei {
            Iei::ConcatenatedShortMessages8bitReference => {
                if udh_length == 5 && ieidl == 3 {
                    Ok(Self::ConcatenatedShortMessages(ConcatenatedShortMessages {
                        reference: *source.get_result(3)? as _,
                        total_messages: *source.get_result(4)?,
                        message_number: *source.get_result(5)?,
                    }))
                } else {
                    Err(format!(
                        "UDHL must be 5 for {:?} IEIDL must be 3 : Got {}",
                        iei, ieidl
                    ))?
                }
            }
            Iei::ConcatenatedShortMessages16bitReference => {
                if udh_length == 6 && ieidl == 4 {
                    Ok(Self::ConcatenatedShortMessages(ConcatenatedShortMessages {
                        reference: u16::from_be_bytes([
                            *source.get_result(3)?,
                            *source.get_result(4)?,
                        ]),
                        total_messages: *source.get_result(5)?,
                        message_number: *source.get_result(6)?,
                    }))
                } else {
                    Err(format!(
                        "UDHL must be 6 for {:?} IEIDL must be 4 : Got {}",
                        iei, ieidl
                    ))?
                }
            }
            Iei::ApplicationPortAddressing16bitAddress => {
                if udh_length == 6 && ieidl == 4 {
                    Ok(Self::ApplicationPortAddressing16(
                        ApplicationPortAddressing16 {
                            destination_port: u16::from_be_bytes([
                                *source.get_result(3)?,
                                *source.get_result(4)?,
                            ]),
                            source_port: u16::from_be_bytes([
                                *source.get_result(5)?,
                                *source.get_result(6)?,
                            ]),
                        },
                    ))
                } else {
                    Err(format!(
                        "UDHL must be 6 for {:?} IEIDL must be 4 : Got {}",
                        iei, ieidl
                    ))?
                }
            }
        }
    }
}

#[rustfmt::skip]
impl fmt::Display for Udh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "   ├─ UDH      ")?;
        writeln!(f, "   │  ├─ Length                : {} bytes", self.len)?;
        write!(f, "   │  └─ Content               : {}", self.content)
    }
}

#[rustfmt::skip]
impl fmt::Display for UdhContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConcatenatedShortMessages(csm) => write!(f, "{csm}"),
            Self::ApplicationPortAddressing16(apa) => write!(f, "{apa}"),
        }
    }
}

#[rustfmt::skip]
impl fmt::Display for ConcatenatedShortMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Concatenated SMS: reference=0x{:04X}, part={}/{}",
            self.reference,
            self.message_number,
            self.total_messages,
        )
    }
}

#[rustfmt::skip]
impl fmt::Display for ApplicationPortAddressing16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Application Port Addressing 16-bit: destination_port={}, source_port={}",
            self.destination_port,
            self.source_port,
        )
    }
}
