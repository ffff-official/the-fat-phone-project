//! ```text
//! The TP-Data-Coding-Scheme field, defined in GSM 03.40, indicates the data coding scheme of the TP-UD field, and may indicate a message class.
//! Any reserved codings will be assumed to be the GSM default alphabet (the same as codepoint 00000000) by a receiving entity.
//! The octet is used according to a coding group which is indicated in bits 7..4. The octet is then coded as follows
//! ```
use num_enum::TryFromPrimitive;

use crate::Error;

#[derive(Debug)]
pub(crate) enum TpDcs {
    DataCoding(DataCoding),
    #[allow(unused)]
    MessageWaitingIndication(MessageWaitingIndication),
    Reserved,
}

#[derive(Debug)]
pub(crate) struct DataCoding {
    #[allow(unused)]
    pub compressed: bool,
    pub alphabet: Alphabet,
    #[allow(unused)]
    pub class: Option<MessageClass>,
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) struct MessageWaitingIndication {
    pub method: IndicationMethod,
    pub alphabet: Alphabet,
    pub status: IndicationStatus,
    pub origin: IndicationType,
}

impl TryFrom<u8> for TpDcs {
    type Error = Error;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0x00..=0x3F => Self::DataCoding(DataCoding {
                compressed: v & 0b0010_0000 != 0,
                alphabet: ((v & 0b0000_1100) >> 2).try_into()?,
                class: match (v & 0b0001_0000) != 0 {
                    false => None,
                    true => Some((v & 0b0000_0011).try_into()?),
                },
            }),
            0x40..=0xBF => Self::Reserved,
            0xC0..=0xCF => Self::MessageWaitingIndication(MessageWaitingIndication {
                method: IndicationMethod::DiscardMessage,
                alphabet: Alphabet::default(),
                status: ((v & 0b0000_1000) >> 3).try_into()?,
                origin: (v & 0b0000_0011).try_into()?,
            }),
            0xD0..=0xDF => Self::MessageWaitingIndication(MessageWaitingIndication {
                method: IndicationMethod::StoreMessage,
                alphabet: Alphabet::default(),
                status: ((v & 0b0000_1000) >> 3).try_into()?,
                origin: (v & 0b0000_0011).try_into()?,
            }),
            0xE0..=0xEF => Self::MessageWaitingIndication(MessageWaitingIndication {
                method: IndicationMethod::StoreMessage,
                alphabet: Alphabet::Ucs2,
                status: ((v & 0b0000_1000) >> 3).try_into()?,
                origin: (v & 0b0000_0011).try_into()?,
            }),
            0xF0..=0xFF => Self::DataCoding(DataCoding {
                compressed: false,
                alphabet: ((v & 0b0000_0100) >> 2).try_into()?,
                class: Some((v & 0b0000_0011).try_into()?),
            }),
        })
    }
}

#[derive(Debug, TryFromPrimitive, Default)]
#[repr(u8)]
pub(crate) enum Alphabet {
    #[default]
    Gsm7 = 0,
    Data8Bit = 1,
    Ucs2 = 2,
    Reserved = 3,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum MessageClass {
    /// Immediate display (alert)
    Class0 = 0,
    /// ME specific
    Class1 = 1,
    /// SIM specific
    Class2 = 2,
    /// TE specific
    Class3 = 3,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum IndicationMethod {
    DiscardMessage = 0,
    StoreMessage = 1,
}

/// Ehere a message is received with bits 7..4 set to 1101, the mobile shall store the text of the SMS message in addition to setting the indication.
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum IndicationStatus {
    SetIndicationInactive = 0,
    SetIndicationActive = 1,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum IndicationType {
    Voicemail = 0,
    FaxMessage = 1,
    Email = 2,
    Other = 3,
}
