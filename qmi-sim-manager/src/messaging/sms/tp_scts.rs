use std::fmt;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::Error;

pub(crate) const TIMESTAMP_LEN: usize = 7;

#[derive(Debug)]
pub(crate) struct TpScts(pub u64);

impl TryFrom<&[u8]> for TpScts {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != TIMESTAMP_LEN {
            return Err(format!("invalid TP-SCTS length: {}", value.len()).into());
        }

        let year = decode_bcd_swapped(value[0]);
        let month = decode_bcd_swapped(value[1]);
        let day = decode_bcd_swapped(value[2]);
        let hour = decode_bcd_swapped(value[3]);
        let minute = decode_bcd_swapped(value[4]);
        let second = decode_bcd_swapped(value[5]);

        let tz_byte = value[6];
        let tz_sign_negative = (tz_byte & 0x08) != 0;

        let tz_quarters = ((tz_byte & 0x07) * 10 + (tz_byte >> 4)) as i32;
        let tz_offset_seconds = tz_quarters * 15 * 60;

        let tz_offset_seconds = if tz_sign_negative {
            -tz_offset_seconds
        } else {
            tz_offset_seconds
        };

        let full_year = if year >= 70 {
            1900 + year as i32
        } else {
            2000 + year as i32
        };

        let naive = NaiveDate::from_ymd_opt(full_year, month, day)
            .ok_or("invalid TP-SCTS date")?
            .and_hms_opt(hour, minute, second)
            .ok_or("invalid TP-SCTS time")?;

        let utc = naive - chrono::Duration::seconds(tz_offset_seconds as i64);
        Ok(TpScts(Utc.from_utc_datetime(&utc).timestamp() as u64))
    }
}

fn decode_bcd_swapped(byte: u8) -> u32 {
    ((byte & 0x0F) * 10 + (byte >> 4)) as u32
}

#[rustfmt::skip]
impl fmt::Display for TpScts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dt = DateTime::<Utc>::from_timestamp(self.0 as i64, 0);
        match dt {
            Some(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M:%S UTC")),
            None => write!(f, "invalid timestamp"), 
        }
    }
}
