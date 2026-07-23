use crate::{Error, messaging::mms::m_retrieve_conf::Charset};

use super::*;

use uuid::Uuid;
use xmltree::{Element, EmitterConfig, XMLNode};

const ADDRESS_PREFIX: &str = "+";
const ADDRESS_SUFIX: &str = "/TYPE=PLMN\0";

pub(crate) fn new(content: &[MmsContent], from: Option<&str>, to: &[&str]) -> SmResult<Vec<u8>> {
    let mut output = Vec::new();

    // HEADER
    output.push(XMmsFieldName::MessageType as u8 | 0x80);
    output.push(MessageType::MSendReq as u8);
    output.push(XMmsFieldName::TransactionId as u8 | 0x80);
    let reference = format!("{}\0", Uuid::new_v4().to_string());
    output.extend_from_slice(reference.as_bytes());
    output.push(XMmsFieldName::Version as u8 | 0x80);
    output.push(0x92); // version 1.2

    if let Some(from) = from {
        // ALTERNATIVE 1
        output.push(XMmsFieldName::From as u8 | 0x80);
        output.push((from.len() + ADDRESS_PREFIX.len() + ADDRESS_SUFIX.len() + 1) as u8);
        output.push(0x80);
        output.extend_from_slice(ADDRESS_PREFIX.as_bytes());
        output.extend_from_slice(from.as_bytes());
        output.extend_from_slice(ADDRESS_SUFIX.as_bytes());
    } else {
        // ALTERNATIVE 2
        output.push(XMmsFieldName::From as u8 | 0x80);
        output.push(0x01);
        output.push(0x81);
    }

    for dest in to {
        output.push(XMmsFieldName::To as u8 | 0x80);
        output.extend_from_slice(ADDRESS_PREFIX.as_bytes());
        output.extend_from_slice(dest.as_bytes());
        output.extend_from_slice(ADDRESS_SUFIX.as_bytes());
    }
    output.push(XMmsFieldName::ContentType as u8 | 0x80);
    output.push(0x1B); // The following len is 27 bytes
    output.push(WspWellKnownContentType::WapMultipartRelated as u8 | 0x80);
    output.push(0x89); // type parameter
    output.extend_from_slice("application/smil\0".as_bytes());
    output.push(0x8A); // start parameter
    output.extend_from_slice("<smil>\0".as_bytes());

    drop(crate::messaging::tracing_debug_hex(&output));

    let mut xmms_field = XMmsFields::default();
    let mut idx = 0;

    while idx < output.len() && parse_xmms_field(&output, &mut xmms_field, &mut idx)? {}
    tracing::debug!("\n{}", xmms_field);

    // CONTENT
    output.push(content.len() as u8 + 1); // 2, 3, 4 etc... parts

    let smil_content = new_smil_boilerplate(content)?;
    tracing::debug!("{}", smil_content);

    fn create_header(
        output: &mut Vec<u8>,
        content_len: usize,
        content_type: WspContentType,
        index: usize,
    ) -> SmResult<()> {
        use WspWellKnownContentType::*;
        let (content_name, charset, content_id, content_disp) = match &content_type {
            WspContentType::WspWellKnownContentType(ImageJpeg) => (
                format!("image{:06}.jpg", index),
                None,
                format!("<image{:06}>", index),
                format!("image{:06}.jpg", index),
            ),
            WspContentType::WspWellKnownContentType(TextPlain) => (
                format!("text{:06}.txt", index),
                Some(Charset::Utf8),
                format!("<text{:06}>", index),
                format!("text{:06}.txt", index),
            ),
            WspContentType::Custom(custom) => match custom.as_str() {
                "audio/mp4" => (
                    format!("recording{:06}.m4a", index),
                    Some(Charset::Utf8),
                    format!("<recording{:06}>", index),
                    format!("recording{:06}.m4a", index),
                ),
                "application/smil" => (
                    "smil.xml".to_string(),
                    None,
                    "\"<smil>".to_string(),
                    "smil.xml".to_string(),
                ),
                _ => Err(Error::Other(format!(
                    "content_type {:?} is not implemented",
                    content_type
                )))?,
            },
            _ => Err(Error::Other(
                format!("content_type {:?} is not implemented", content_type).into(),
            ))?,
        };

        let content_type_len = match &content_type {
            WspContentType::Custom(s) => s.len() + 1,
            WspContentType::WspWellKnownContentType(_) => 1,
        };
        let content_header_len = content_type_len + content_name.len() + 1 + 1;
        let header_len = content_header_len + content_id.len() + content_disp.len() + 2 + 2 + 1;

        write_uintvar(output, header_len as _);
        write_uintvar(output, content_len as _);
        write_uintvar(output, content_header_len as _);
        match &content_type {
            WspContentType::Custom(s) => {
                output.extend_from_slice(s.as_bytes());
                output.push(b'\0');
            }
            WspContentType::WspWellKnownContentType(b) => {
                output.push(*b as u8 | 0x80);
            }
        }
        if let Some(charset) = charset {
            output.push(WspPartHeaders::CONTENT_CHARSET);
            output.push(charset as u8 | 0x80);
        }
        output.push(WspPartHeaders::CONTENT_NAME);
        output.extend_from_slice(content_name.as_bytes());
        output.push(b'\0');
        output.push(WspPartHeaders::CONTENT_ID);
        output.extend_from_slice(content_id.as_bytes());
        output.push(b'\0');
        output.push(WspPartHeaders::CONTENT_DISPOSITION);
        output.extend_from_slice(content_disp.as_bytes());
        output.push(b'\0');
        Ok(())
    }

    create_header(
        &mut output,
        smil_content.len(),
        WspContentType::Custom("application/smil".into()),
        0,
    )?;
    output.extend_from_slice(smil_content.as_bytes());

    for (idx, media) in content.iter().enumerate() {
        create_header(
            &mut output,
            media.content.len(),
            media.content_type.clone(),
            idx + 1,
        )?;
        output.extend_from_slice(&media.content);
    }

    let result = m_retrieve_conf::WspMRetrieveConf::try_from(output.as_slice())?;
    tracing::debug!("{}", result);

    tracing::debug!("PDU LEN = {}", output.len());
    Ok(output)
}

fn new_smil_boilerplate(content: &[MmsContent]) -> SmResult<String> {
    let config = EmitterConfig::new()
        .write_document_declaration(false)
        .perform_indent(false)
        .pad_self_closing(false);

    let mut smil = Element::new("smil");
    let mut head = Element::new("head");
    let mut layout = Element::new("layout");
    let root_layout = Element::new("root-layout");
    layout.children.push(XMLNode::Element(root_layout));

    use WspWellKnownContentType::*;
    if content
        .iter()
        .find(|c| c.content_type == WspContentType::WspWellKnownContentType(ImageJpeg))
        .is_some()
    {
        let mut region = Element::new("region");

        region.attributes.insert("id".into(), "Image".into());
        region.attributes.insert("fit".into(), "meet".into());
        region.attributes.insert("top".into(), "0".into());
        region.attributes.insert("left".into(), "0".into());
        region.attributes.insert("height".into(), "100%".into());
        region.attributes.insert("width".into(), "100%".into());

        layout.children.push(XMLNode::Element(region));
    }
    if content
        .iter()
        .find(|c| c.content_type == WspContentType::WspWellKnownContentType(TextPlain))
        .is_some()
    {
        let mut region = Element::new("region");

        region.attributes.insert("id".into(), "Text".into());
        region.attributes.insert("top".into(), "*80%".into());
        region.attributes.insert("left".into(), "0".into());
        region.attributes.insert("height".into(), "20%".into());
        region.attributes.insert("width".into(), "100%".into());

        layout.children.push(XMLNode::Element(region));
    }

    head.children.push(XMLNode::Element(layout));
    smil.children.push(XMLNode::Element(head));

    let mut body = Element::new("body");

    for (idx, media) in content.iter().enumerate() {
        let mut par = Element::new("par");
        match media.content_type {
            WspContentType::WspWellKnownContentType(ImageJpeg) => {
                par.attributes.insert("dur".into(), "5000ms".into());
                let mut img = Element::new("img");
                img.attributes
                    .insert("src".into(), format!("image{:06}.jpg", idx));
                img.attributes.insert("region".into(), "Image".into());
                par.children.push(XMLNode::Element(img));
            }
            WspContentType::WspWellKnownContentType(TextPlain) => {
                par.attributes.insert("dur".into(), "5000ms".into());
                let mut text = Element::new("text");
                text.attributes
                    .insert("src".into(), format!("text{:06}.txt", idx));
                text.attributes.insert("region".into(), "Text".into());
                par.children.push(XMLNode::Element(text));
            }
            // Some()
            // Some(MmsContentType::AudioMp4) => {
            //     par.attributes.insert("dur".into(), "5000ms".into()); // TODO sound duration
            //     let mut audio = Element::new("audio");
            //     audio
            //         .attributes
            //         .insert("src".into(), format!("recording{:06}.m4a", idx));
            //     audio.attributes.insert("dur".into(), "5000ms".into()); // TODO sound duration
            //     par.children.push(XMLNode::Element(audio));
            // }
            // Some(MmsContentType::ApplicationSmilXml) => unimplemented!(),
            _ => unimplemented!(),
        }
        body.children.push(XMLNode::Element(par));
    }

    smil.children.push(XMLNode::Element(body));

    let mut out = Vec::new();
    smil.write_with_config(&mut out, config)?;
    let xml = String::from_utf8(out)?;
    tracing::debug!("{}", xml);
    Ok(xml)
}

fn write_uintvar(buf: &mut Vec<u8>, mut value: u32) {
    let mut tmp = [0u8; 5];
    let mut pos = 5;

    tmp[pos - 1] = (value & 0x7F) as u8;
    pos -= 1;
    value >>= 7;

    while value != 0 {
        tmp[pos - 1] = ((value & 0x7F) as u8) | 0x80;
        pos -= 1;
        value >>= 7;
    }

    buf.extend_from_slice(&tmp[pos..]);
}
