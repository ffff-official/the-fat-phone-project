//! ```text
//! Wireless Session Protocol (WSP) is a protocol from the WAP (Wireless Application Protocol) suite designed to provide efficient
//! session and content handling over low-bandwidth mobile networks. It defines compact binary encodings for headers, content types,
//! parameters, and multipart message bodies to reduce network overhead. MMS reuses several WSP encoding mechanisms,
//! such as well-known content types, variable-length integers, and multipart body structures.
//!
//! M-Retrieve.conf
//! │
//! ├─ MMS headers
//! │   ├─ X-Mms-Message-Type
//! │   ├─ X-Mms-Transaction-Id
//! │   ├─ X-Mms-MMS-Version
//! │   ├─ From
//! │   ├─ To
//! │   ├─ Date
//! │   └─ Content-Type  ← Last MMS Header
//! │
//! └─ MMS body
//!     └─ multipart
//!         ├─ Part 1
//!         ├─ Part 2
//!         └─ ...
//! WSP Part
//! ├─ headers
//! │   ├─ Content-Type
//! │   ├─ Content-Location
//! │   ├─ Content-ID
//! │   └─ Content-Disposition
//! │
//! └─ body
//!     └─ [binary data slice]
//! ```
use crate::{Error, SmResult};

use super::*;

use std::cmp::Ordering;
use std::fmt;

impl<'a> TryFrom<&'a [u8]> for WspMRetrieveConf<'a> {
    type Error = Error;
    fn try_from(source: &'a [u8]) -> Result<Self, Self::Error> {
        let (xmms_fields, source) = super::set_xmms_fields(source)?;
        let mut idx: usize = 0;
        let body = match &xmms_fields.content_type {
            Some(ContentType::WapMultipartRelated(WapMultipartRelatedHeader {
                start: _,
                mime_type: _,
            })) => {
                let nbr_parts = *source.get_result(idx)?;
                tracing::debug!("Got {} parts", nbr_parts);
                let mut parts: Vec<WspPart> = Vec::new();
                idx += 1;
                while idx < source.len() {
                    parts.push(get_part(source, &mut idx)?);
                }
                if idx > source.len() {
                    tracing::warn!("IDX > source.len() {} > {}", idx, source.len());
                }
                WspBody::WapMultiPart(WspMultipart {
                    kind: WspMultipartKind::Related,
                    parts,
                })
            }
            Some(ContentType::ImageJpeg) => {
                // IMPORTANT : NOT CHECKED
                WspBody::ImageJpeg(source)
            }
            Some(ContentType::TextPlain) => {
                // IMPORTANT : NOT CHECKED
                WspBody::TextPlain(source)
            }
            None => Err("Content-Type must be given ar this point")?,
        };
        Ok(Self { xmms_fields, body })
    }
}

fn get_part<'a>(source: &'a [u8], idx: &mut usize) -> SmResult<WspPart<'a>> {
    let (header_len, idx_offset) =
        read_uintvar(source.get_result(*idx..)?).ok_or("cannot get header len")?;
    *idx += idx_offset;

    let (content_len, content_offset) =
        read_uintvar(source.get_result(*idx..)?).ok_or("cannot get content len")?;
    *idx += content_offset;
    tracing::debug!("");
    tracing::debug!("Header len = {} Content len = {}", header_len, content_len);
    let begin_header = *idx;
    let begin_body = begin_header + header_len as usize;
    let next_part = begin_body + content_len as usize;
    let header_raw_data = source.get_result(begin_header..begin_body)?;

    // The bellow content-header length represents ONLY `content-type + content-name` | `content-type + content charset + content-name`
    // The (others field) total length is calculated like that `header_len` - `content-header length`
    // Value-length =
    //     Short-length
    //   | (Length-quote Uintvar-integer)
    //
    // Short-length = 0..30
    // Length-quote = 31 (0x1F)
    // 0x1F means that the following value is the length encoded as a Uintvar-integer.
    let mut calcultated_content_header_len = 0; // Coherency check
    let mut content_header_len = *source.get_result(*idx)? as usize;
    *idx += 1;

    if content_header_len == 0x1f {
        let (new_len, content_header_offset) =
            read_uintvar(source.get_result(*idx..)?).ok_or("cannot get header len")?;
        content_header_len = new_len as _;
        *idx += content_header_offset;
    }
    tracing::debug!("Content-header-len = {}", content_header_len);

    let old_idx = *idx; // coherency check
    let content_type_value = *source.get_result(*idx)?;
    let content_type = if content_type_value & 0x80 != 0 {
        *idx += 1;
        WspContentType::WspWellKnownContentType(WspWellKnownContentType::try_from(
            content_type_value & 0x7F,
        )?)
    } else {
        WspContentType::Custom(string_until_nul(source, idx)?)
    };
    tracing::debug!("Content_type = {}", content_type);
    calcultated_content_header_len += *idx - old_idx; // coherency check

    let mut content_charset: Option<Charset> = None;
    let mut content_name: Option<String> = None;
    let mut content_id: Option<String> = None;
    let mut content_disposition: Option<String> = None;

    while *idx < begin_body {
        let value = *source.get_result(*idx)?;
        tracing::debug!("Content Header value = {:02x?}", value);
        let old_idx = *idx; // coherency check
        *idx += 1;
        match value {
            WspPartHeaders::CONTENT_CHARSET => {
                let res = read_integer(source.get_result(*idx..)?);
                match res {
                    Some((res, offset)) => {
                        content_charset = Some(Charset::try_from(res).unwrap_or_default());
                        tracing::debug!("Content_charset = {:?}", content_charset);
                        *idx += offset;
                        calcultated_content_header_len += *idx - old_idx; // coherency check
                    }
                    None => {
                        tracing::warn!("Read UintVar has failed !");
                        break;
                    }
                }
            }
            WspPartHeaders::CONTENT_NAME => {
                content_name = Some(string_until_nul(source, idx)?);
                tracing::debug!("Content_name = {:?}", content_name);
                calcultated_content_header_len += *idx - old_idx; // coherency check
            }
            WspPartHeaders::CONTENT_DISPOSITION => {
                content_disposition = Some(string_until_nul(source, idx)?);
                tracing::debug!("Content_disposition = {:?}", content_disposition);
            }
            WspPartHeaders::CONTENT_ID => {
                content_id = Some(string_until_nul(source, idx)?);
                tracing::debug!("Content_id = {:?}", content_id);
            }
            _ => {
                tracing::warn!("Unexpected content value : {}", value);
                break;
            }
        }
    }

    // coherency check
    if calcultated_content_header_len != content_header_len as usize {
        tracing::warn!(
            "calcultated_content_header_len != content_header_len {} != {}",
            calcultated_content_header_len,
            content_header_len
        );
    }
    match (*idx).cmp(&begin_body) {
        Ordering::Equal => tracing::debug!("IDX correctly aligned with body start"),
        Ordering::Less => tracing::warn!("IDX < begin_body {} > {}", idx, begin_body),
        Ordering::Greater => tracing::warn!("IDX > begin_body {} > {}", idx, begin_body),
    }

    match (content_name, content_id, content_disposition) {
        (Some(content_name), Some(content_id), Some(content_disposition)) => {
            *idx = next_part;
            Ok(WspPart {
                headers: WspPartHeaders {
                    content_type,
                    content_charset,
                    content_name,
                    content_id,
                    content_disposition,
                    raw_data: header_raw_data,
                },
                body: source.get_result(begin_body..next_part)?,
            })
        }
        _ => Err("Missing Content")?,
    }
}

#[derive(Debug)]
pub(crate) struct WspMRetrieveConf<'a> {
    pub xmms_fields: XMmsFields,
    pub body: WspBody<'a>,
}

#[derive(Debug)]
pub(crate) enum WspBody<'a> {
    WapMultiPart(WspMultipart<'a>),
    TextPlain(&'a [u8]), // IMPORTANT : NOT CHECKED
    ImageJpeg(&'a [u8]), // IMPORTANT : NOT CHECKED
}

#[derive(Debug)]
pub(crate) struct WspMultipart<'a> {
    pub kind: WspMultipartKind,
    pub parts: Vec<WspPart<'a>>,
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) enum WspMultipartKind {
    Related,
    Mixed,
    Alternative,
}

#[derive(Debug)]
pub(crate) struct WspPart<'a> {
    pub headers: WspPartHeaders<'a>,
    pub body: &'a [u8],
}

#[derive(Debug)]
pub(crate) struct WspPartHeaders<'a> {
    pub content_type: WspContentType,
    pub content_charset: Option<Charset>,
    pub content_name: String,
    pub content_id: String,
    pub content_disposition: String,
    pub raw_data: &'a [u8],
}

impl<'a> WspPartHeaders<'a> {
    pub const CONTENT_CHARSET: u8 = 0x81;
    pub const CONTENT_NAME: u8 = 0x85;
    pub const CONTENT_ID: u8 = 0xC0;
    pub const CONTENT_DISPOSITION: u8 = 0x8E;
}

#[derive(Debug, TryFromPrimitive, Default, Copy, Clone)]
#[repr(u32)]
pub(crate) enum Charset {
    #[default]
    Unknown = 0x00,
    Usascii = 0x03,
    Iso88591 = 0x04,
    Utf8 = 0x6A,
    Utf16 = 0x3F7,
    Windows1252 = 0x8CC,
}

/// 8.1.2 Variable Length Unsigned Integers
/// Many fields in the data unit formats are of variable length. Typically, there will be an associated field that specifies the size
/// of the variable length field. In order to keep the data unit formats as small as possible, a variable length unsigned integer
/// encoding is used to specify lengths. The larger the unsigned integer, the larger the size of its encoding.
///
/// to encode a large unsigned integer, split it into 7-bit fragments and place them in the payloads of multiple octets. The most
/// significant bits are placed in the first octets with the least significant bits ending up in the last octet. All octets MUST set the
/// Continue bit to 1 except the last octet, which MUST set the Continue bit to 0
///
/// For example, the number 0x87A5 (1000 0111 1010 0101) is encoded in three octets as shown in Figure 28
/// [1+0000101] [1+0011111] [0+0100101]
fn read_uintvar(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u32;

    for (i, &b) in bytes.iter().take(5).enumerate() {
        value = (value << 7) | u32::from(b & 0x7F);

        if b & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}
fn read_integer(bytes: &[u8]) -> Option<(u32, usize)> {
    let first = *bytes.first()?;

    // Short-integer
    if first & 0x80 == 0x80 {
        return Some(((first & 0x7F) as u32, 1));
    }

    // Long-integer
    let len = first as usize;

    if len == 0 || len > 4 || bytes.len() < len + 1 {
        return None;
    }

    let mut value = 0u32;

    for &b in &bytes[1..=len] {
        value = (value << 8) | u32::from(b);
    }

    Some((value, len + 1))
}

impl fmt::Display for WspMRetrieveConf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "M-Retrieve.conf")?;
        writeln!(f)?;
        writeln!(f, "MMS/Headers:")?;
        writeln!(f, "{}", self.xmms_fields)?;
        writeln!(f)?;
        writeln!(f, "Body:")?;
        write!(f, "{}", self.body)
    }
}

impl fmt::Display for WspBody<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WspBody::WapMultiPart(mp) => write!(f, "{mp}"),
            WspBody::TextPlain(data) => {
                writeln!(f, "Text/plain")?;
                write!(f, "  {} bytes", data.len())
            }
            WspBody::ImageJpeg(data) => {
                writeln!(f, "Image/jpeg")?;
                write!(f, "  {} bytes", data.len())
            }
        }
    }
}

impl fmt::Display for WspMultipart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Multipart ({}) - {} part(s)",
            self.kind,
            self.parts.len()
        )?;

        for (idx, part) in self.parts.iter().enumerate() {
            writeln!(f)?;
            writeln!(f, "Part #{}", idx + 1)?;

            let rendered = format!("{part}");

            for line in rendered.lines() {
                writeln!(f, "  {line}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for WspMultipartKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WspMultipartKind::Related => write!(f, "related"),
            WspMultipartKind::Mixed => write!(f, "mixed"),
            WspMultipartKind::Alternative => write!(f, "alternative"),
        }
    }
}

fn dump(f: &mut fmt::Formatter<'_>, source: &[u8], columns: usize, lines: usize) -> fmt::Result {
    for (i, chunk) in source.chunks(columns).enumerate().take(lines) {
        write!(f, "{:08x}: ", i * columns)?;

        for b in chunk {
            write!(f, "{:02x} ", b)?;
        }
        writeln!(f)?;
    }
    Ok(())
}

impl fmt::Display for WspPart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = 8;
        let columns = 32;
        let displayed_body_len = lines * columns;
        writeln!(f, "WSP Part")?;
        writeln!(f, "  headers:")?;
        writeln!(f, "{}", self.headers)?;
        if self.body.len() > displayed_body_len {
            writeln!(
                f,
                "  body: {} bytes (only the begining of the sequence is displayed)",
                self.body.len()
            )?;
        } else {
            writeln!(f, "  body: {} bytes", self.body.len())?;
        }
        dump(f, self.body, columns, lines)
    }
}

impl fmt::Display for WspPartHeaders<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  ├─ Content-Type: {}", self.content_type)?;
        writeln!(f, "  ├─ Content-Charset: {:?}", self.content_charset)?;
        writeln!(f, "  ├─ Content-Name: {}", self.content_name)?;
        writeln!(f, "  ├─ Content-ID: {}", self.content_id)?;
        writeln!(f, "  ├─ Content-Disposition: {}", self.content_disposition)?;
        writeln!(f, "  └─ raw headers: {} bytes", self.raw_data.len())?;
        dump(f, self.raw_data, 32, 8)
    }
}

impl fmt::Display for WspContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WspContentType::WspWellKnownContentType(ct) => write!(f, "{ct}"),
            WspContentType::Custom(s) => write!(f, "custom = {s}"),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/mms/m_retrieve_conf.rs"]
mod tests;
