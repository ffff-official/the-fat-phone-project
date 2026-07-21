//! See <http://rfc.nop.hu/sms/default.htm>
//! ```text
//! TPDU SMS SUBMIT
//! ├─ 0x00 SMSC informations length : Is already knowing, just set 0x00
//! ├─ TP-MTI / first byte (TPDU Header)
//! ├─ TP-Message-Reference. The "00" value here lets the phone set the message reference number itself.
//! ├─ TP-RA   (Remote Address) cf. length / type of address and phone number 0B + 91 + 64 07 28 15 53 F8 (2..20 numbers)
//! ├─ TP-PID  (Protocol Identifier)
//! ├─ TP-DCS  (Data Coding Scheme)
//! ├─ TP-VP   (TP-Validity-Period. "AA" means 4 days. Note: This octet is optional, see bits 4 and 3 of the first octet)
//! ├─ TP-UDL  (User Data Length)
//! └─ TP-UD   (User Data)
//!     ├─ UDH (optional, if TP-UDHI=1)
//!     └─ User payload
//! ```
use crate::SmError;

use super::*;

use bitfields::bitfield;
use gsm7_rs::Gsm7Writer;

use std::mem::size_of;
use std::sync::atomic::{AtomicU8, Ordering};

static MESSAGE_REFERENCE: AtomicU8 = AtomicU8::new(0);

const MAX_SMS_TP_UD_BYTE_LEN: usize = 140;
const MAX_SMS_TP_UD_GSM7_SEPTETS: usize = 140 * 8 / 7; // 160
const MULTI_SMS_8B_UDL: usize = 6;
const TOTAL_MSG_UDL_OFFSET: usize = 4;

/// That's a real boilerplate !
#[bitfield([u8; 3])]
struct SmsDeliverTpduHeaderBegin {
    #[bits(8, default = 0x00)]
    zero: u8,
    #[bits(8, default = TpduHeaderSmsSubmit::from_bits(0x11) )]
    tp_mti: TpduHeaderSmsSubmit,
    #[bits(8, default = 0x00)]
    tp_message_reference: u8,
}

/// That's a real boilerplate !
#[bitfield([u8; 4])]
struct SmsDeliverTpduHeaderEnd {
    #[bits(8, default = 0x00)]
    tp_pid: u8, // Ignore it ! It is a dummy enum
    #[bits(8, default = 0x00)]
    tp_dcs: u8, // Important ! The correct DSC pattern may be calculated
    #[bits(8, default = 0xAA)]
    tp_validity_period: u8,
    #[bits(8, default = 0x00)]
    tp_udl: u8,
}

pub(crate) fn get_sms_deliver_tpdu(
    phone_number: &str,
    content: &str,
    recept_ack: bool,
) -> SmResult<Vec<Vec<u8>>> {
    let mut header_begin = SmsDeliverTpduHeaderBegin::default();
    let mut tpdu_header = TpduHeaderSmsSubmit::new();
    tpdu_header.set_tp_mti(Mti::SMS_SUBMIT);
    tpdu_header.set_tp_vpf(Vpf::RELATIVE);
    tpdu_header.set_tp_srr(if recept_ack { 1 } else { 0 }); // reception ACK
    header_begin.set_tp_mti(tpdu_header);

    let mut header_end = SmsDeliverTpduHeaderEnd::default();

    let mut type_of_address = TypeOfAddress::default();
    type_of_address.set_numbering_plan_identification(
        NumberingPlanIdentification::ISDN_TELEPHONE_NUMBERING_PLAN,
    );
    type_of_address.set_type_of_number(TypeOfNumber::INTERNATIONAL_NUMBER);
    let tp_ra = TpAddress::new(phone_number, type_of_address);

    let message_reference = MESSAGE_REFERENCE.fetch_add(1, Ordering::Relaxed); // used for mutipart messages
    let mut writer = Gsm7Writer::new(Vec::new()); // Trying GSM7
    Ok(match writer.write_str(content) {
        Ok(_) => {
            let nb_septets = writer.get_written_bit_count() / 7;
            // For a single SMS, the maximum payload is 160 septets. However, for a concatenated (multi-part) SMS,
            // the UDH set on 8 bits reference (User Data Header on 6 bytes) used for segmentation reduces the available payload
            // to 153 septets per segment.

            // If the last septet of a segment is the beginning of a GSM 7-bit extended character (which requires 2 septets),
            // that character must be moved entirely to the next segment. In that case, the maximum payload of the current
            // segment is effectively reduced to 152 septets.

            // In this case, the current segment can contain at most 152 septets of payload, leaving one septet unused to avoid
            // splitting a GSM 7-bit extended character across segments.
            if nb_septets <= MAX_SMS_TP_UD_GSM7_SEPTETS {
                header_end.set_tp_udl(nb_septets as u8);

                let mut sms = Vec::new();
                sms.extend_from_slice(header_begin.into_le_bytes().as_slice());
                sms.extend_from_slice(&tp_ra.to_bytes()?);
                sms.extend_from_slice(header_end.into_le_bytes().as_slice());
                sms.extend_from_slice(gsm(writer.into_writer())?.as_slice());

                vec![sms]
            } else {
                tracing::debug!("nb_septets = {}", nb_septets);
                let mut tp_mti = header_begin.tp_mti(); // Anounce that a UDL part we be present into the message
                tp_mti.set_tp_udhi(1);
                header_begin.set_tp_mti(tp_mti);

                let mut writer = gsm(Gsm7Writer::new_with_bit_offset(Vec::new(), 1))?; // Quick and dirty. Recreate a new buffer and reset read.
                let mut gsm7_offset = 0;
                let mut sms_array = vec![];

                #[rustfmt::skip]
                let create_sms = |writer: Gsm7Writer<Vec<u8>>, current_part: usize|
                 -> SmResult<Vec<u8>> {
                    let mut sms = Vec::new();

                    let mut local_header_end = header_end;
                    local_header_end.set_tp_udl((writer.get_written_bit_count() / 7) as u8 + MULTI_SMS_8B_UDL as u8 + 1); // + UDH size + 1 (7 bits)

                    sms.extend_from_slice(header_begin.into_le_bytes().as_slice());
                    sms.extend_from_slice(&tp_ra.to_bytes()?);
                    sms.extend_from_slice(local_header_end.into_le_bytes().as_slice());

                    // Put UDH segment
                    sms.push(0x05); // UDH length
                    sms.push(0x00); // 8 bits reference
                    sms.push(0x03); // IEDL length of the UDH following content
                    sms.push(message_reference); // Message reference
                    sms.push(0xff); // Total part --- IMPORTANT : Must be UPDATED as last operation
                    sms.push(current_part as u8 + 1); // curent part

                    sms.extend_from_slice(gsm(writer.into_writer())?.as_slice());

                    Ok(sms)
                };

                for c in content.chars() {
                    //dbg!(gsm7_offset);
                    match gsm7_rs::is_extended_character(c) {
                        false => {
                            if gsm7_offset >= MAX_SMS_TP_UD_GSM7_SEPTETS - 7 {
                                sms_array.push(create_sms(writer, sms_array.len())?);

                                gsm7_offset = 0;
                                writer = gsm(Gsm7Writer::new_with_bit_offset(Vec::new(), 1))?;
                            }
                            gsm(writer.write_char(c))?;
                            gsm7_offset += 1;
                        }
                        true => {
                            if gsm7_offset >= MAX_SMS_TP_UD_GSM7_SEPTETS - 8 {
                                sms_array.push(create_sms(writer, sms_array.len())?);

                                gsm7_offset = 0;
                                writer = gsm(Gsm7Writer::new_with_bit_offset(Vec::new(), 1))?;
                            }
                            gsm(writer.write_char(c))?;
                            gsm7_offset += 2;
                        }
                    }
                }
                if gsm7_offset != 0 {
                    sms_array.push(create_sms(writer, sms_array.len())?);
                }
                // Set Total part value into all UDL segments
                let total_parts = sms_array.len() as u8;
                let idx = size_of::<SmsDeliverTpduHeaderBegin>()
                    + size_of::<SmsDeliverTpduHeaderEnd>()
                    + tp_ra.to_bytes()?.len()
                    + TOTAL_MSG_UDL_OFFSET;
                for sms in &mut sms_array {
                    sms[idx] = total_parts;
                }
                sms_array
            }
        }
        Err(_e) => {
            // So it is UTF16/UCS2 msg
            let complete_le_body = content.encode_utf16().collect::<Vec<u16>>();
            header_end.set_tp_dcs(0x08); // USC2 / UTF8
            let mut sms_array = vec![];
            let mut body: Vec<u8> = Vec::new();

            #[rustfmt::skip]
            let mut create_sms = |body: &mut Vec<u8>, current_part: usize, is_multipart: bool| -> SmResult<Vec<u8>> {
                let mut sms = Vec::new();

                if is_multipart {
                    let mut tp_mti = header_begin.tp_mti(); // Anounce that a UDL part we be present into the message
                    tp_mti.set_tp_udhi(1);
                    header_begin.set_tp_mti(tp_mti);
                    header_end.set_tp_udl((body.len() + MULTI_SMS_8B_UDL) as _); // + UDH size
                } else {
                    header_end.set_tp_udl(body.len() as _);
                }

                sms.extend_from_slice(header_begin.into_le_bytes().as_slice());
                sms.extend_from_slice(&tp_ra.to_bytes()?);
                sms.extend_from_slice(header_end.into_le_bytes().as_slice());

                if is_multipart {
                    // Put UDH segment
                    sms.push(0x05); // UDH length
                    sms.push(0x00); // 8 bits reference
                    sms.push(0x03); // IEDL length of the UDH following content
                    sms.push(message_reference); // Message reference
                    sms.push(0xff); // Total part --- IMPORTANT : Must be UPDATED as last operation
                    sms.push(current_part as u8 + 1); // curent part
                }
                sms.append(body);
                Ok(sms)
            };

            if complete_le_body.len() <= MAX_SMS_TP_UD_BYTE_LEN / 2 {
                let mut body = complete_le_body
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect::<Vec<u8>>();

                sms_array.push(create_sms(&mut body, 0, false)?);
            } else {
                for v in complete_le_body {
                    // high surrogate : U+D800 to U+DBFF. 0
                    if ((v & 0xFC00) == 0xD800
                        && body.len() == MAX_SMS_TP_UD_BYTE_LEN - MULTI_SMS_8B_UDL - 2)
                        || (body.len() == MAX_SMS_TP_UD_BYTE_LEN - MULTI_SMS_8B_UDL)
                    {
                        sms_array.push(create_sms(&mut body, sms_array.len(), true)?);
                    }
                    body.extend_from_slice(&v.to_be_bytes());
                }

                if !body.is_empty() {
                    sms_array.push(create_sms(&mut body, sms_array.len(), true)?);
                }
                // Set Total part value into all UDL segments
                let total_parts = sms_array.len() as u8;
                let idx = size_of::<SmsDeliverTpduHeaderBegin>()
                    + size_of::<SmsDeliverTpduHeaderEnd>()
                    + tp_ra.to_bytes()?.len()
                    + TOTAL_MSG_UDL_OFFSET;
                for sms in &mut sms_array {
                    sms[idx] = total_parts;
                }
            }
            sms_array
        }
    })
}

fn gsm<T>(res: Result<T, std::io::Error>) -> SmResult<T> {
    res.map_err(SmError::Gsm7Error)
}

#[cfg(test)]
#[path = "../../../tests/sms/sms_submit.rs"]
mod tests;
