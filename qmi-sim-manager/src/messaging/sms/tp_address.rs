//! The address, whether it is the sender or the sender, is always coded in this way below.
//! In one case we are talking about tp_oa for Origin and in the other tp_ra for Remote.
//! +----------------------+-------------------+----------------------+
//! | Address-Length (1 B) | Type-of-Address   | Address-Value        |
//! |                      | (1 B)             | (0..10 B)            |
//! +----------------------+-------------------+----------------------+
//! TP-OA (origin address) cf. SMS-DELIVER
//! ├── Address-Length (1 byte)
//! ├── Type-of-Address (1 byte)
//! └── Address-Value (0..10 bytes)
//! TP-RA (remote address) cf. SMS-STATUS-REPORT
//! ├── Address-Length (1 byte)
//! ├── Type-of-Address (1 byte)
//! └── Address-Value (0..10 bytes)

use bitfields::{bitfield, bitflag};
use gsm7_rs::Gsm7Reader;

use crate::{SmError, SmResult, messaging::SliceResultExt};

use std::io;

#[derive(Debug)]
pub(crate) struct TpAddress {
    pub type_of_address: TypeOfAddress,
    pub address: String,
}

impl TpAddress {
    pub(crate) fn new(address: &str, type_of_address: TypeOfAddress) -> Self {
        Self {
            type_of_address,
            address: address.into(),
        }
    }

    pub(crate) fn to_bytes(&self) -> SmResult<Vec<u8>> {
        let length = self.address.len();
        let mut output = vec![0u8; 2];
        output[0] = length as u8;
        output[1] = self.type_of_address.into_bits();
        let mut encoded_address = vec![0u8; (length / 2) + (length % 2)];
        for (idx_str, c) in self.address.chars().enumerate() {
            if !(c as u8 >= b'0' && c as u8 <= b'9') {
                Err(SmError::InvalidDestination(
                    "contains non numerical character".to_string(),
                ))?
            }
            let idx_oa_address = idx_str / 2;
            let number = c as u8 - b'0';
            if idx_str % 2 == 0 {
                encoded_address[idx_oa_address] |= number;
            } else {
                encoded_address[idx_oa_address] |= number << 4;
            }
        }
        if let Some(last) = encoded_address.last_mut()
            && length != 0
            && length % 2 == 1
        {
            *last |= 0xf0;
        }

        output.extend_from_slice(&encoded_address);
        Ok(output)
    }

    pub(crate) fn from_bytes(s: &[u8]) -> SmResult<(Self, usize)> {
        let length = *s.get_result(0)? as usize;
        let type_of_address = TypeOfAddress::from_bits(*s.get_result(1)?);
        tracing::debug!("type_of_number = {:?}", type_of_address.type_of_number());
        let encoded_len = length / 2 + length % 2;
        let address = match type_of_address.type_of_number() {
            TypeOfNumber::INTERNATIONAL_NUMBER => {
                let mut address = String::new();
                for i in 2..(2 + encoded_len) {
                    let value = *s.get_result(i)?;
                    address.push((b'0' + (value & 0x0f)) as char);
                    let t = value >> 4;
                    if t != 0xf {
                        address.push((b'0' + t) as char);
                    }
                }
                address
            }
            TypeOfNumber::ALPHANUMERIC_NUMBER => {
                let reader = Gsm7Reader::new(io::Cursor::new(s.get_result(2..2 + encoded_len)?));
                reader
                    .collect::<io::Result<String>>()
                    .map_err(SmError::Gsm7Error)?
            }
            _ => Err("Unhandled Type of Address")?,
        };
        Ok((
            Self {
                type_of_address,
                address,
            },
            encoded_len + 2,
        ))
    }
}

#[bitfield(u8)]
pub(crate) struct TypeOfAddress {
    #[bits(4, default = NumberingPlanIdentification::ISDN_TELEPHONE_NUMBERING_PLAN)]
    pub numbering_plan_identification: NumberingPlanIdentification,
    #[bits(3)]
    pub type_of_number: TypeOfNumber,
    #[bits(1, default = 1)]
    pub always_true: u8,
}

/// The Numbering-plan-identification applies for Type-of-number = 000, 001 and 010.
/// For Type-of-number = 101 bits 3,2,1,0 are reserved and shall be transmitted as 0000.
/// Note that for addressing any of the entities SC, MSC, SGSN or MS,
/// Numbering-plan-identification = 0001 will always be used.
/// However, for addressing the SME, any specified Numbering-plan-identification value may be used.
#[bitflag(u8)]
#[allow(non_camel_case_types)]
pub(crate) enum NumberingPlanIdentification {
    UNKNOWN = 0b0000,
    #[base]
    ISDN_TELEPHONE_NUMBERING_PLAN = 0b0001,
    DATA_NUMBERING_PLAN = 0b0011,
    TELEX_NUMBERING_PLAN = 0b0100,
    NATIONAL_NUMBERING_PLAN = 0b1000,
    PRIVATE_NUMBERING_PLAN = 0b1001,
    ERMES_NUMBERING_PLAN = 0b1010,
    RESERVED_FOR_EXTENSION = 0b1111,
}

#[derive(Debug)]
#[bitflag(u8)]
#[allow(non_camel_case_types)]
pub(crate) enum TypeOfNumber {
    /// This is used when the user or network has no a priori information about the numbering plan. In this case, the Address-Value field is organized according to the network dialling plan, e.g. prefix or escape digits might be present.
    UNKNOWN = 0b000,
    #[base]
    INTERNATIONAL_NUMBER = 0b001,
    /// Prefix or escape digits shall not be included.
    NATIONAL_NUMBER = 0b010,
    /// This is used to indicate administration/service number specific to the serving network, e.g. used to access an operator.
    NETWORK_SPECIFIC_NUMBER = 0b011,
    /// This is used when a specific short number representation is stored in one or more SCs as part of a higher layer application. (Note that "Subscriber number" shall only be used in connection with the proper PID referring to this application).
    SUBSCRIBER_NUMBER = 0b100,
    /// (coded according to GSM TS 03.38 7-bit default alphabet)
    ALPHANUMERIC_NUMBER = 0b101,
    ABBREVIATED_NUMBER = 0b110,
    RESERVED_FOR_EXTENSION = 0b111,
}
