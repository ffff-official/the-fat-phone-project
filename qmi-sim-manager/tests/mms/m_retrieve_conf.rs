use super::*;
use crate::configure_tracing;

use serial_test::serial;

#[test]
fn read_uintvar() {
    assert_eq!(super::read_uintvar(&[0x00]), Some((0, 1)));
    assert_eq!(super::read_uintvar(&[0x2F]), Some((47, 1)));
    assert_eq!(super::read_uintvar(&[0x81, 0x55]), Some((213, 2)));
    assert_eq!(super::read_uintvar(&[0x8B, 0x91, 0x33]), Some((182451, 3)));
}

#[test]
fn read_integer() {
    assert_eq!(super::read_integer(&[0xEA]), Some((106, 1))); // UTF-8
    assert_eq!(super::read_integer(&[0x84]), Some((4, 1))); // ISO-8859-1
    assert_eq!(super::read_integer(&[0x02, 0x03, 0xF7]), Some((1015, 3))); // UTF-16
    assert_eq!(super::read_integer(&[0x02, 0x08, 0xCC]), Some((2252, 3))); // Windows-1252
    assert_eq!(super::read_integer(&[]), None);
    assert_eq!(super::read_integer(&[0x02]), None);
}

const M_RETRIEVE_CONF_HEADER: [u8; 145] = [
    0x8c, 0x84, 0x98, 0x6e, 0x56, 0x6e, 0x47, 0x53, 0x58, 0x63, 0x43, 0x45, 0x66, 0x47, 0x65, 0x34,
    0x67, 0x41, 0x6d, 0x75, 0x66, 0x71, 0x36, 0x39, 0x41, 0x00, 0x8d, 0x92, 0x8b, 0x6e, 0x56, 0x6e,
    0x47, 0x53, 0x58, 0x63, 0x43, 0x45, 0x66, 0x47, 0x65, 0x34, 0x67, 0x41, 0x6d, 0x75, 0x66, 0x71,
    0x36, 0x39, 0x41, 0x00, 0x85, 0x04, 0x6a, 0x47, 0xed, 0xe5, 0x89, 0x18, 0x80, 0x2b, 0x33, 0x33,
    0x36, 0x34, 0x31, 0x32, 0x38, 0x36, 0x36, 0x30, 0x33, 0x2f, 0x54, 0x59, 0x50, 0x45, 0x3d, 0x50,
    0x4c, 0x4d, 0x4e, 0x00, 0x97, 0x2b, 0x33, 0x33, 0x37, 0x32, 0x31, 0x32, 0x37, 0x37, 0x35, 0x32,
    0x33, 0x2f, 0x54, 0x59, 0x50, 0x45, 0x3d, 0x50, 0x4c, 0x4d, 0x4e, 0x00, 0x8a, 0x80, 0x8f, 0x81,
    0x86, 0x81, 0x90, 0x81, 0x84, 0x1b, 0xb3, 0x8a, 0x3c, 0x73, 0x6d, 0x69, 0x6c, 0x3e, 0x00, 0x89,
    0x61, 0x70, 0x70, 0x6c, 0x69, 0x63, 0x61, 0x74, 0x69, 0x6f, 0x6e, 0x2f, 0x73, 0x6d, 0x69, 0x6c,
    0x00,
];

#[test]
#[serial]
fn mms_m_retrieve_conf_header() {
    configure_tracing();
    println!("{}", set_xmms_fields(&M_RETRIEVE_CONF_HEADER).unwrap().0);
}

const M_RETRIEVE_CONF: &[u8] = include_bytes!("../../data-test/mms.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF).unwrap();
    println!("{}", result);
}

const M_RETRIEVE_CONF_AZIZ: &[u8] = include_bytes!("../../data-test/mms-aziz.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete_aziz() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF_AZIZ).unwrap();
    println!("{}", result);
}

const M_RETRIEVE_CONF_MIXED_TXT_IMG_M4A: &[u8] =
    include_bytes!("../../data-test/mms-mixed-1-image-1-sound-1-text.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete_mixed_txt_img_m4a() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF_MIXED_TXT_IMG_M4A).unwrap();
    println!("{}", result);
}

const M_RETRIEVE_CONF_MIXED_IMG_M4A: &[u8] =
    include_bytes!("../../data-test/mms-mixed-1-image-1-sound.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete_mixed_img_m4a() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF_MIXED_IMG_M4A).unwrap();
    println!("{}", result);
}

const M_RETRIEVE_CONF_MIXED_2_IMG: &[u8] = include_bytes!("../../data-test/mms-mixed-2-image.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete_mixed_2_img() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF_MIXED_2_IMG).unwrap();
    println!("{}", result);
}

const M_RETRIEVE_CONF_MIXED_M4A: &[u8] = include_bytes!("../../data-test/mms-mixed-sound.bin");

#[test]
#[serial]
fn mms_m_retrieve_conf_complete_mixed_m4a() {
    configure_tracing();
    let result = WspMRetrieveConf::try_from(M_RETRIEVE_CONF_MIXED_M4A).unwrap();
    println!("{}", result);
}
